use chia_protocol::Message;

#[derive(Clone)]
pub enum PeerMessage {
    Protocol(Message),
    Close,
}
