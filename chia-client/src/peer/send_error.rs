use std::fmt;

#[derive(Debug)]
pub enum SendError {
    StreamError { reason: String },
    SocketError { reason: String },
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StreamError { reason } | Self::SocketError { reason } => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for SendError {}
