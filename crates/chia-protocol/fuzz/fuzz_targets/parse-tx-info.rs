#![no_main]
use chia_protocol::TransactionsInfo;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = TransactionsInfo::from_bytes(data);
});
