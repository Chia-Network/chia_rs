#![no_main]
use libfuzzer_sys::fuzz_target;

use chia::gen::conditions::{parse_spends, MempoolVisitor};
use clvmr::allocator::Allocator;
use fuzzing_utils::{make_tree, BitCursor};

use chia::gen::flags::{COND_ARGS_NIL, NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);
    for flags in &[0, COND_ARGS_NIL, STRICT_ARGS_COUNT, NO_UNKNOWN_CONDS] {
        let _ret = parse_spends::<MempoolVisitor>(&a, input, 33000000000, *flags);
    }
});
