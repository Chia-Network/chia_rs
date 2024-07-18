use chia_protocol::ProtocolMessageTypes;
use semver::Version;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;

use crate::Event;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Peer is missing certificate")]
    MissingCertificate,

    #[error("Handshake not received")]
    ExpectedHandshake,

    #[error("Invalid protocol version {0}")]
    InvalidProtocolVersion(String),

    #[error("Wrong network id {0}")]
    WrongNetworkId(String),

    #[error("Outdated protocol version {0}, expected {1}")]
    OutdatedProtocolVersion(Version, Version),

    #[error("Streamable error: {0}")]
    Streamable(#[from] chia_traits::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tungstenite::Error),

    #[error("TLS error: {0}")]
    Tls(#[from] native_tls::Error),

    #[error("Unexpected message received with type {0:?}")]
    UnexpectedMessage(ProtocolMessageTypes),

    #[error("Expected response with type {0:?}, found {1:?}")]
    InvalidResponse(Vec<ProtocolMessageTypes>, ProtocolMessageTypes),

    #[error("Failed to send event")]
    EventNotSent,

    #[error("Failed to send message")]
    Send(#[from] SendError<Event>),

    #[error("Failed to receive message")]
    Recv(#[from] RecvError),

    #[error("Connection timeout: {0}")]
    ConnectionTimeout(Elapsed),

    #[error("Handshake timeout: {0}")]
    HandshakeTimeout(Elapsed),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
