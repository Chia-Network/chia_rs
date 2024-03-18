use chia_streamable_macro::streamable;

#[streamable]
pub struct TimestampedPeerInfo {
    host: String,
    port: u16,
    timestamp: u64,
}
