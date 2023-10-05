#![no_main]
use chia_protocol::HeaderBlock;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let _ret = <HeaderBlock as Streamable>::parse(&mut Cursor::<&[u8]>::new(data));
});
