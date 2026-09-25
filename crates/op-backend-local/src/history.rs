use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use op_backend::{
    Actor, BackendError, Change, ChangeKind, LogEntry, LogQuery, MemorySnapshot, Revision,
    RevisionId, Timestamp,
};
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior, params};
use sha2::{Digest as _, Sha256};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS revisions (
    id      INTEGER PRIMARY KEY,
    parent  INTEGER,
    at      TEXT NOT NULL,
    author  TEXT NOT NULL,
    email   TEXT,
    via     TEXT,
    message TEXT NOT NULL,
    settled INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS blobs (
    hash  TEXT PRIMARY KEY,
    bytes BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS changes (
    revision INTEGER NOT NULL REFERENCES revisions (id),
    path     TEXT NOT NULL,
    kind     TEXT NOT NULL,
    blob     TEXT REFERENCES blobs (hash),
    PRIMARY KEY (revision, path)
);
CREATE INDEX IF NOT EXISTS changes_by_path ON changes (path, revision);
";

const BUSY: Duration = Duration::from_secs(5);

pub(crate) struct History {
    db: Connection,
}

pub(crate) struct Recorded {
    pub revision: Revision,
    pub changes: Vec<Change>,
}

impl History {
    pub fn open(path: &Path) -> Result<Self, BackendError> {
        let db = Connection::open(path).map_err(storage)?;
        db.busy_timeout(BUSY).map_err(storage)?;
        db.pragma_update(None, "journal_mode", "WAL")
            .map_err(storage)?;
        db.execute_batch(SCHEMA).map_err(storage)?;
        Ok(Self { db })
    }

    pub fn latest(&self) -> Result<Option<i64>, BackendError> {
        self.db
            .query_row("SELECT MAX(id) FROM revisions", [], |row| row.get(0))
            .map_err(storage)
    }

    pub fn unsettled(&self) -> Result<Vec<i64>, BackendError> {
        let mut statement = self
            .db
            .prepare("SELECT id FROM revisions WHERE settled = 0 ORDER BY id")
            .map_err(storage)?;
        let ids = statement
            .query_map([], |row| row.get(0))
            .map_err(storage)?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(storage)?;
        Ok(ids)
    }

    pub fn settle(&self, id: i64) -> Result<(), BackendError> {
        self.db
            .execute("UPDATE revisions SET settled = 1 WHERE id = ?1", [id])
            .map_err(storage)?;
        Ok(())
    }

    pub fn exists(&self, id: i64) -> Result<bool, BackendError> {
        self.db
            .query_row("SELECT 1 FROM revisions WHERE id = ?1", [id], |_| Ok(()))
            .optional()
            .map(|found| found.is_some())
            .map_err(storage)
    }

    pub fn snapshot(&self, id: Option<i64>) -> Result<MemorySnapshot, BackendError> {
        let Some(id) = id else {
            return Ok(MemorySnapshot::default());
        };
        let mut statement = self
            .db
            .prepare(
                "SELECT c.path, b.bytes FROM changes c JOIN blobs b ON b.hash = c.blob
                 WHERE c.revision = (
                     SELECT MAX(c2.revision) FROM changes c2
                     WHERE c2.path = c.path AND c2.revision <= ?1
                 )",
            )
            .map_err(storage)?;
        let files = statement
            .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(storage)?
            .collect::<Result<BTreeMap<String, Vec<u8>>, _>>()
            .map_err(storage)?;
        Ok(MemorySnapshot::new(Some(revision_id(id)), files))
    }

    // `None` when another writer moved the history past `expected` since the caller read it. A
    // recorded revision stays unsettled until the files on disk match it, and `open` finishes one
    // that a crash interrupted.
    pub fn record(
        &mut self,
        expected: Option<i64>,
        author: &Actor,
        at: Timestamp,
        message: &str,
        writes: &BTreeMap<String, Option<Vec<u8>>>,
        kinds: &BTreeMap<String, ChangeKind>,
    ) -> Result<Option<Recorded>, BackendError> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let parent: Option<i64> = tx
            .query_row("SELECT MAX(id) FROM revisions", [], |row| row.get(0))
            .map_err(storage)?;
        if parent != expected {
            return Ok(None);
        }
        tx.execute(
            "INSERT INTO revisions (parent, at, author, email, via, message) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![parent, at.to_string(), author.name, author.email, author.via, message],
        )
        .map_err(storage)?;
        let id = tx.last_insert_rowid();
        let mut changes = Vec::new();
        for (path, content) in writes {
            let kind = kinds[path];
            let hash = match content {
                Some(bytes) => {
                    let hash = hash(bytes);
                    tx.execute(
                        "INSERT OR IGNORE INTO blobs (hash, bytes) VALUES (?1, ?2)",
                        params![hash, bytes],
                    )
                    .map_err(storage)?;
                    Some(hash)
                }
                None => None,
            };
            tx.execute(
                "INSERT INTO changes (revision, path, kind, blob) VALUES (?1, ?2, ?3, ?4)",
                params![id, path, kind_name(kind), hash],
            )
            .map_err(storage)?;
            changes.push(Change::new(path.clone(), kind));
        }
        tx.commit().map_err(storage)?;
        Ok(Some(Recorded {
            revision: Revision {
                id: revision_id(id),
                parents: parent.map(revision_id).into_iter().collect(),
                author: author.clone(),
                at,
                message: message.to_owned(),
            },
            changes,
        }))
    }

    pub fn revision(&self, id: i64) -> Result<Revision, BackendError> {
        self.db
            .query_row(
                "SELECT parent, at, author, email, via, message FROM revisions WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, String>(1)?,
                        Actor {
                            name: row.get(2)?,
                            email: row.get(3)?,
                            via: row.get(4)?,
                        },
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .map_err(storage)
            .and_then(|(parent, at, author, message)| {
                Ok(Revision {
                    id: revision_id(id),
                    parents: parent.map(revision_id).into_iter().collect(),
                    author,
                    at: at.parse().map_err(storage)?,
                    message,
                })
            })
    }

    pub fn changed_paths(&self, id: i64) -> Result<Vec<String>, BackendError> {
        let mut statement = self
            .db
            .prepare("SELECT path FROM changes WHERE revision = ?1 ORDER BY path")
            .map_err(storage)?;
        let paths = statement
            .query_map([id], |row| row.get(0))
            .map_err(storage)?
            .collect::<Result<Vec<String>, _>>()
            .map_err(storage)?;
        Ok(paths)
    }

    pub fn log(&self, query: &LogQuery) -> Result<Vec<LogEntry>, BackendError> {
        let before = match &query.before {
            Some(id) if self.exists(parse_id(id)?)? => Some(parse_id(id)?),
            Some(id) => return Err(BackendError::UnknownRevision(id.clone())),
            None => None,
        };
        let limit = query
            .limit
            .map_or(-1, |limit| i64::try_from(limit).unwrap_or(i64::MAX));
        let mut statement = self
            .db
            .prepare(
                "SELECT r.id, r.parent, r.at, r.author, r.email, r.via, r.message FROM revisions r
                 WHERE (?2 IS NULL OR r.id < ?2)
                   AND EXISTS (
                       SELECT 1 FROM changes c
                       WHERE c.revision = r.id AND substr(c.path, 1, length(?1)) = ?1
                   )
                 ORDER BY r.id DESC
                 LIMIT ?3",
            )
            .map_err(storage)?;
        let rows = statement
            .query_map(params![query.prefix, before, limit], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    Actor {
                        name: row.get(3)?,
                        email: row.get(4)?,
                        via: row.get(5)?,
                    },
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        let mut entries = Vec::new();
        for (id, parent, at, author, message) in rows {
            entries.push(LogEntry {
                revision: Revision {
                    id: revision_id(id),
                    parents: parent.map(revision_id).into_iter().collect(),
                    author,
                    at: at.parse().map_err(storage)?,
                    message,
                },
                changes: self.changes_under(id, &query.prefix)?,
            });
        }
        Ok(entries)
    }

    fn changes_under(&self, id: i64, prefix: &str) -> Result<Vec<Change>, BackendError> {
        let mut statement = self
            .db
            .prepare(
                "SELECT path, kind FROM changes
                 WHERE revision = ?1 AND substr(path, 1, length(?2)) = ?2
                 ORDER BY path",
            )
            .map_err(storage)?;
        let rows = statement
            .query_map(params![id, prefix], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        rows.into_iter()
            .map(|(path, kind)| Ok(Change::new(path, parse_kind(&kind)?)))
            .collect()
    }
}

pub(crate) fn revision_id(id: i64) -> RevisionId {
    RevisionId::new(id.to_string())
}

pub(crate) fn parse_id(id: &RevisionId) -> Result<i64, BackendError> {
    id.as_str()
        .parse()
        .map_err(|_| BackendError::UnknownRevision(id.clone()))
}

fn kind_name(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "added",
        ChangeKind::Modified => "modified",
        ChangeKind::Removed => "removed",
    }
}

fn parse_kind(name: &str) -> Result<ChangeKind, BackendError> {
    match name {
        "added" => Ok(ChangeKind::Added),
        "modified" => Ok(ChangeKind::Modified),
        "removed" => Ok(ChangeKind::Removed),
        other => Err(BackendError::Storage(format!(
            "the history names an unknown change kind {other:?}"
        ))),
    }
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn storage(err: impl std::fmt::Display) -> BackendError {
    BackendError::storage(err)
}
