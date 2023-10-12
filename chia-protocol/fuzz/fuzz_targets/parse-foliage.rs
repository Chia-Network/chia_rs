#![no_main]
use chia_protocol::Foliage;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = Foliage::from_bytes(data);
});
