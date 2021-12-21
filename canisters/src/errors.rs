pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Agent error: {0}")]
    Agent(#[from] ic_agent::agent::agent_error::AgentError),

    #[error("Identity error: {0}")]
    Ident(#[from] ic_agent::identity::PemError),

    #[error("Failed to get config directory")]
    MissingConfig,
}
