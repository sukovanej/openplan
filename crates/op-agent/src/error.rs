#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("cannot start `{binary}`: {source}")]
    Spawn {
        binary: String,
        #[source]
        source: std::io::Error,
    },
    #[error("the agent has stopped")]
    Stopped,
}
