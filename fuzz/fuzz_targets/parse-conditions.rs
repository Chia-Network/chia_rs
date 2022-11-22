#![no_main]
use libfuzzer_sys::fuzz_target;

use clvmr::allocator::Allocator;
use chia::fuzzing_utils::{BitCursor, make_tree};
use chia::gen::conditions::parse_spends;

use chia::gen::flags::{COND_ARGS_NIL, STRICT_ARGS_COUNT, NO_UNKNOWN_CONDS};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);
    for flags in &[0, COND_ARGS_NIL, STRICT_ARGS_COUNT, NO_UNKNOWN_CONDS] {
        let _ret = parse_spends(&a, input, 33000000000, *flags);
    }
});

