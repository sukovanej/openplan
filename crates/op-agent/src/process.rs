use std::process::Stdio;

use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::mpsc,
};

use crate::error::AgentError;

const CAPACITY: usize = 256;

// Both CLIs speak one JSON value per line over stdio and log to stderr.
pub struct Process<I, O> {
    pub incoming: mpsc::Receiver<O>,
    pub outgoing: mpsc::Sender<I>,
    pub child: Child,
}

pub fn spawn<I, O>(mut command: Command) -> Result<Process<I, O>, AgentError>
where
    I: Serialize + Send + 'static,
    O: DeserializeOwned + Send + 'static,
{
    let binary = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| AgentError::Spawn {
            binary: binary.clone(),
            source,
        })?;

    let mut stdin = child.stdin.take().expect("stdin is piped");
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");

    let (line_tx, incoming) = mpsc::channel(CAPACITY);
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str(&line) {
                Ok(message) => {
                    if line_tx.send(message).await.is_err() {
                        break;
                    }
                }
                // Neither CLI publishes a schema and both change the wire format between patch
                // releases, so one unreadable line must never end the session.
                Err(error) => tracing::warn!(%error, line, "unreadable agent output"),
            }
        }
    });

    let (outgoing, mut to_write) = mpsc::channel::<I>(CAPACITY);
    tokio::spawn(async move {
        while let Some(message) = to_write.recv().await {
            let Ok(mut line) = serde_json::to_vec(&message) else {
                tracing::error!("cannot encode a message for the agent");
                continue;
            };
            line.push(b'\n');
            if stdin.write_all(&line).await.is_err() || stdin.flush().await.is_err() {
                break;
            }
        }
    });

    let logged = binary;
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::debug!(binary = %logged, line, "agent log");
        }
    });

    Ok(Process {
        incoming,
        outgoing,
        child,
    })
}
