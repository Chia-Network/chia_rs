use chia_protocol::Message;
use chia_traits::chia_error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error<R> {
    #[error("{0:?}")]
    Chia(#[from] chia_error::Error),

    #[error("{0}")]
    WebSocket(#[from] tungstenite::Error),

    #[error("{0:?}")]
    InvalidResponse(Message),

    #[error("missing response")]
    MissingResponse,

    #[error("rejection")]
    Rejection(R),
}
