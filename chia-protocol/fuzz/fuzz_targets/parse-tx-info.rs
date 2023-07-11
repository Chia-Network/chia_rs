#![no_main]
use chia_protocol::Streamable;
use chia_protocol::TransactionsInfo;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let _ret = <TransactionsInfo as Streamable>::parse(&mut Cursor::<&[u8]>::new(data));
});
