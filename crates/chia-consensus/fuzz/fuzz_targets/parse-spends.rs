#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::gen::conditions::{parse_spends, MempoolVisitor};
use chia_fuzz::{make_list, BitCursor};
use clvmr::{Allocator, NodePtr};

use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::gen::flags::{NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_list(&mut a, &mut BitCursor::new(data));
    // spends is a list of spends
    let input = a.new_pair(input, NodePtr::NIL).unwrap();
    for flags in &[0, STRICT_ARGS_COUNT, NO_UNKNOWN_CONDS] {
        let _ret =
            parse_spends::<MempoolVisitor>(&a, input, 33_000_000_000, *flags, &TEST_CONSTANTS);
    }
});
