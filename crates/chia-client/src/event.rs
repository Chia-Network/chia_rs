use chia_protocol::Message;

use crate::PeerId;

#[derive(Debug, Clone)]
pub enum Event {
    Message(PeerId, Message),
    ConnectionClosed(PeerId),
}
