use chia_protocol::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    Protocol(Message),
}
