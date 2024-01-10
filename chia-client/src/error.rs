use chia_protocol::Message;
use chia_traits::chia_error;
use thiserror::Error;

pub type Result<T, Rejection> = std::result::Result<T, Error<Rejection>>;

#[derive(Debug, Error)]
pub enum Error<Rejection> {
    #[error("{0:?}")]
    Chia(#[from] chia_error::Error),

    #[error("{0}")]
    WebSocket(#[from] tungstenite::Error),

    #[error("{0:?}")]
    InvalidResponse(Message),

    #[error("missing response")]
    MissingResponse,

    #[error("rejection")]
    Rejection(Rejection),
}
