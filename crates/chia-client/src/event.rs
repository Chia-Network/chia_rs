use chia_protocol::{CoinStateUpdate, Handshake, NewPeakWallet};

#[derive(Debug, Clone)]
pub enum Event {
    Handshake(Handshake),
    NewPeakWallet(NewPeakWallet),
    CoinStateUpdate(CoinStateUpdate),
}
