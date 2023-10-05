#![no_main]
use chia_protocol::Program;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let _ret = <Program as Streamable>::parse(&mut Cursor::<&[u8]>::new(data));
});
