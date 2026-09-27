use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, mpsc};
use std::thread::JoinHandle;

use op_agent::{AgentKind, Transcript};
use op_task::Timestamp;
use rusqlite::{Connection, params};
use serde_json::Value;

pub const AGENT_SESSIONS_FILE: &str = "agent-sessions.sqlite3";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA: &str = "
BEGIN;
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    project TEXT NOT NULL,
    agent TEXT NOT NULL,
    title TEXT NOT NULL,
    context TEXT,
    tasks TEXT NOT NULL,
    started_at TEXT NOT NULL,
    transcript TEXT NOT NULL,
    done_at TEXT
) STRICT;
PRAGMA user_version = 1;
COMMIT;
";

#[derive(Debug, thiserror::Error)]
pub enum AgentStoreError {
    #[error("the agent session store could not be opened, read, or written: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("an agent session could not be encoded for the store: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("the agent session store has schema {found}, and this daemon reads {SCHEMA_VERSION}")]
    Schema { found: i64 },
}

// A session as the store keeps it: what the list and the chat show, and in the transcript the
// agent's own session id, which a resumed agent takes up again.
#[derive(Debug, Clone)]
pub struct StoredSession {
    pub id: String,
    pub project: String,
    pub agent: AgentKind,
    pub title: String,
    pub context: Option<String>,
    pub tasks: Vec<String>,
    pub started_at: Timestamp,
    pub transcript: Transcript,
}

enum Write {
    Save(Box<StoredSession>),
    Done { id: String, at: Timestamp },
}

// One writer thread owns the connection, so the event pump never waits on the disk, and a burst of
// events for one session costs one write.
pub struct AgentStore {
    writes: Mutex<Option<mpsc::Sender<Write>>>,
    writer: Mutex<Option<JoinHandle<()>>>,
}

impl AgentStore {
    // The sessions not yet marked done come back with the store.
    pub fn open(path: &Path) -> Result<(Self, Vec<StoredSession>), AgentStoreError> {
        Self::start(Connection::open(path)?)
    }

    pub fn in_memory() -> Self {
        let connection = Connection::open_in_memory().expect("SQLite opens a memory database");
        Self::start(connection)
            .expect("a new memory database takes the schema")
            .0
    }

    fn start(connection: Connection) -> Result<(Self, Vec<StoredSession>), AgentStoreError> {
        migrate(&connection)?;
        let open = read_open(&connection)?;
        let (writes, received) = mpsc::channel();
        let writer = std::thread::Builder::new()
            .name("agent-store".to_owned())
            .spawn(move || write_all(connection, received))
            .expect("the system starts a thread");
        let store = Self {
            writes: Mutex::new(Some(writes)),
            writer: Mutex::new(Some(writer)),
        };
        Ok((store, open))
    }

    pub fn save(&self, session: StoredSession) {
        self.send(Write::Save(Box::new(session)));
    }

    pub fn done(&self, id: &str, at: Timestamp) {
        self.send(Write::Done {
            id: id.to_owned(),
            at,
        });
    }

    fn send(&self, write: Write) {
        if let Some(writes) = self.writes.lock().expect("store lock poisoned").as_ref() {
            let _ = writes.send(write);
        }
    }

    // Every write sent before this is on disk when it returns. It blocks on the writer thread.
    pub fn close(&self) {
        drop(self.writes.lock().expect("store lock poisoned").take());
        if let Some(writer) = self.writer.lock().expect("store lock poisoned").take() {
            let _ = writer.join();
        }
    }
}

fn migrate(connection: &Connection) -> Result<(), AgentStoreError> {
    // A memory database answers "memory" rather than "wal", which is as good.
    connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
    let found: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match found {
        0 => Ok(connection.execute_batch(SCHEMA)?),
        SCHEMA_VERSION => Ok(()),
        found => Err(AgentStoreError::Schema { found }),
    }
}

// One row the daemon cannot read costs that session, not the daemon's start.
fn read_open(connection: &Connection) -> Result<Vec<StoredSession>, AgentStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, project, agent, title, context, tasks, started_at, transcript
         FROM sessions WHERE done_at IS NULL",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Row {
            id: row.get(0)?,
            project: row.get(1)?,
            agent: row.get(2)?,
            title: row.get(3)?,
            context: row.get(4)?,
            tasks: row.get(5)?,
            started_at: row.get(6)?,
            transcript: row.get(7)?,
        })
    })?;
    let mut open = Vec::new();
    for row in rows {
        let row = row?;
        let id = row.id.clone();
        match row.decode() {
            Ok(session) => open.push(session),
            Err(reason) => {
                tracing::warn!(session = %id, %reason, "skipped an agent session the store holds")
            }
        }
    }
    Ok(open)
}

struct Row {
    id: String,
    project: String,
    agent: String,
    title: String,
    context: Option<String>,
    tasks: String,
    started_at: String,
    transcript: String,
}

impl Row {
    fn decode(self) -> Result<StoredSession, String> {
        Ok(StoredSession {
            agent: serde_json::from_value(Value::String(self.agent))
                .map_err(|err| err.to_string())?,
            tasks: serde_json::from_str(&self.tasks).map_err(|err| err.to_string())?,
            started_at: self
                .started_at
                .parse::<Timestamp>()
                .map_err(|err| err.to_string())?,
            transcript: serde_json::from_str(&self.transcript).map_err(|err| err.to_string())?,
            id: self.id,
            project: self.project,
            title: self.title,
            context: self.context,
        })
    }
}

fn write_all(mut connection: Connection, writes: mpsc::Receiver<Write>) {
    while let Ok(first) = writes.recv() {
        let batch: Vec<Write> = std::iter::once(first).chain(writes.try_iter()).collect();
        if let Err(err) = commit(&mut connection, &batch) {
            tracing::error!(error = %err, "the agent session store dropped a write");
        }
    }
}

// A later save of a session carries everything an earlier one in the batch did.
fn commit(connection: &mut Connection, batch: &[Write]) -> Result<(), AgentStoreError> {
    let last: HashMap<&str, usize> = batch
        .iter()
        .enumerate()
        .filter_map(|(at, write)| match write {
            Write::Save(session) => Some((session.id.as_str(), at)),
            Write::Done { .. } => None,
        })
        .collect();
    let transaction = connection.transaction()?;
    for (index, write) in batch.iter().enumerate() {
        match write {
            Write::Save(session) if last.get(session.id.as_str()) == Some(&index) => {
                upsert(&transaction, session)?;
            }
            Write::Save(_) => {}
            Write::Done { id, at } => {
                transaction.execute(
                    "UPDATE sessions SET done_at = ?1 WHERE id = ?2",
                    params![at.to_string(), id],
                )?;
            }
        }
    }
    transaction.commit()?;
    Ok(())
}

// The row's identity never changes after the insert; only what the session did since does.
fn upsert(connection: &Connection, session: &StoredSession) -> Result<(), AgentStoreError> {
    let agent = match serde_json::to_value(session.agent)? {
        Value::String(name) => name,
        other => other.to_string(),
    };
    connection.execute(
        "INSERT INTO sessions (id, project, agent, title, context, tasks, started_at, transcript)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (id) DO UPDATE SET tasks = excluded.tasks, transcript = excluded.transcript",
        params![
            session.id,
            session.project,
            agent,
            session.title,
            session.context,
            serde_json::to_string(&session.tasks)?,
            session.started_at.to_string(),
            serde_json::to_string(&session.transcript)?,
        ],
    )?;
    Ok(())
}
