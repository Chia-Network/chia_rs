#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::gen::conditions::{parse_spends, MempoolVisitor};
use clvmr::{Allocator, NodePtr};
use fuzzing_utils::{make_list, BitCursor};

use chia_consensus::gen::flags::{
    COND_ARGS_NIL, ENABLE_MESSAGE_CONDITIONS, ENABLE_SOFTFORK_CONDITION, NO_UNKNOWN_CONDS,
    STRICT_ARGS_COUNT,
};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_list(&mut a, &mut BitCursor::new(data));
    // spends is a list of spends
    let input = a.new_pair(input, NodePtr::NIL).unwrap();
    for flags in &[
        0,
        COND_ARGS_NIL,
        STRICT_ARGS_COUNT,
        NO_UNKNOWN_CONDS,
        ENABLE_SOFTFORK_CONDITION,
        ENABLE_MESSAGE_CONDITIONS,
    ] {
        let _ret = parse_spends::<MempoolVisitor>(&a, input, 33000000000, *flags);
    }
});
