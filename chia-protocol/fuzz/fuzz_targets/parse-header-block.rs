#![no_main]
use chia_protocol::HeaderBlock;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = HeaderBlock::from_bytes(data);
});
