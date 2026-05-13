use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("authentication required: run `bgg auth`")]
    AuthRequired,

    #[error("BGG queued the request and did not return data after {attempts} retries")]
    QueueTimeout { attempts: u32 },

    #[error("BGG rate limit hit")]
    RateLimited,

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("failed to parse BGG XML: {0}")]
    Parse(String),

    #[error("cache error at {path}: {source}")]
    Cache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("secrets error: {0}")]
    Secrets(String),

    #[error("no cached collection for user {0}; run `bgg sync`")]
    NoCache(String),

    #[error("no logged-in user; run `bgg auth`")]
    NoUser,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::AuthRequired | Error::NoUser => 2,
            _ => 1,
        }
    }
}
