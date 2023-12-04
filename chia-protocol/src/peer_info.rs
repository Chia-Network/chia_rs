use chia_streamable_macro::Streamable;

use crate::streamable_struct;

streamable_struct!(TimestampedPeerInfo {
    host: String,
    port: u16,
    timestamp: u64,
});
