use std::net::{IpAddr, SocketAddr};

use chia_protocol::Message;

use crate::PeerId;

#[derive(Debug, Clone)]
pub enum Event {
    Message(PeerId, Message),
    Connected(PeerId, SocketAddr),
    Disconnected(SocketAddr),
    Banned(IpAddr),
}
