use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", content = "message")]
pub enum ColimaError {
    CommandFailed(String),
    Validation(String),
    NotFound(String),
    Internal(String),
    Network(String),
    Unknown(String),
}

impl fmt::Display for ColimaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColimaError::CommandFailed(m) => write!(f, "Command execution failed: {}", m),
            ColimaError::Validation(m) => write!(f, "Validation error: {}", m),
            ColimaError::NotFound(m) => write!(f, "Not found: {}", m),
            ColimaError::Internal(m) => write!(f, "Internal error: {}", m),
            ColimaError::Network(m) => write!(f, "Network error: {}", m),
            ColimaError::Unknown(m) => write!(f, "Unknown error: {}", m),
        }
    }
}

impl std::error::Error for ColimaError {}

impl From<String> for ColimaError {
    fn from(s: String) -> Self {
        ColimaError::Unknown(s)
    }
}

impl From<&str> for ColimaError {
    fn from(s: &str) -> Self {
        ColimaError::Unknown(s.to_string())
    }
}
