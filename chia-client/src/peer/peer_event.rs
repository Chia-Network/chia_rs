use chia_protocol::{CoinStateUpdate, NewPeakWallet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    CoinStateUpdate(CoinStateUpdate),
    NewPeakWallet(NewPeakWallet),
}
