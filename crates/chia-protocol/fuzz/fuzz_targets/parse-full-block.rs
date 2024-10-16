#![no_main]
use chia_protocol::FullBlock;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = FullBlock::from_bytes(data);
});
