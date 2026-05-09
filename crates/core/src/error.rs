use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("db: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("provider {provider}: {message}")]
    Provider { provider: String, message: String },

    #[error("auth required for provider {0}")]
    AuthRequired(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("keychain: {0}")]
    Keychain(String),

    #[error("internal: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;
