#![no_main]
use chia_protocol::Program;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = Program::from_bytes(data);
});
