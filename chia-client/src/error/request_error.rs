use std::fmt;

use chia_protocol::Message;

#[derive(Debug)]
pub enum RequestError {
    StreamError {
        reason: String,
    },
    SocketError {
        reason: String,
    },
    ResponseError {
        message: Option<Message>,
        reason: String,
    },
    ParseError {
        message: Message,
        reason: String,
    },
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StreamError { reason } | Self::SocketError { reason } => write!(f, "{reason}"),
            Self::ResponseError { message, reason } => match message {
                Some(_) => write!(f, "invalid message: {reason}"),
                None => write!(f, "request failed: {reason}"),
            },
            Self::ParseError { message: _, reason } => {
                write!(f, "could not parse response body: {reason}")
            }
        }
    }
}

impl std::error::Error for RequestError {}
