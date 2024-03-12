use chia_streamable_macro::{streamable, Streamable};

#[streamable]
pub struct TimestampedPeerInfo {
    host: String,
    port: u16,
    timestamp: u64,
}
