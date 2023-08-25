/// Generic result type
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
/// Error type
pub enum Error {
    /// Identity missing from the json
    #[error("Invalid or missing account name in identity config")]
    InvalidOrMissingAccount,

    /// A principal error
    #[error("Principal error: {0}")]
    Principal(#[from] ic_agent::export::PrincipalError),

    /// Standard IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Std env var error
    #[error("Env var error: {0}")]
    EnvVar(#[from] std::env::VarError),

    /// Certificate not found error
    #[error("Certificate not found: {0}")]
    CertNotFound(std::path::PathBuf),

    /// Serde json error
    #[error("Serde error: {0}")]
    Json(#[from] serde_json::Error),

    /// Agent error
    #[error("Agent error: {0}")]
    Agent(#[from] ic_agent::agent::agent_error::AgentError),

    /// Identity error
    #[error("Identity error: {0}")]
    Ident(#[from] ic_agent::identity::PemError),

    /// Missing configuration directory
    #[error("Failed to get config directory")]
    MissingConfig,

    /// Candid error
    #[error("Candid error: {0}")]
    Candid(#[from] candid::Error),

    /// Generic error as a String
    #[error("Generic: {0}")]
    Generic(String),

    /// Percentage error
    #[error("Must be a percent between 0 and 100.")]
    MustBeAPercentage(),

    /// Invalid memory size error
    #[error("Memory allocation must be between 0 and 2^48 (i.e 256TB), inclusively. Got {0}.")]
    InvalidMemorySize(u64),

    /// Command execution failed
    #[error("Command execution failed")]
    CommandExecutionFailed,
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}
