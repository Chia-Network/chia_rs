#![no_main]
use libfuzzer_sys::fuzz_target;

use clvmr::allocator::Allocator;
use chia::fuzzing_utils::{BitCursor, make_tree};
use chia::gen::conditions::parse_args;

use chia::gen::flags::{COND_ARGS_NIL, STRICT_ARGS_COUNT};

use chia::gen::opcodes::{
    AGG_SIG_ME, AGG_SIG_UNSAFE, ALWAYS_TRUE,
    ASSERT_COIN_ANNOUNCEMENT, ASSERT_HEIGHT_ABSOLUTE, ASSERT_HEIGHT_RELATIVE, ASSERT_MY_AMOUNT,
    ASSERT_MY_COIN_ID, ASSERT_MY_PARENT_ID, ASSERT_MY_PUZZLEHASH, ASSERT_PUZZLE_ANNOUNCEMENT,
    ASSERT_SECONDS_ABSOLUTE, ASSERT_SECONDS_RELATIVE, CREATE_COIN, CREATE_COIN_ANNOUNCEMENT,
    CREATE_PUZZLE_ANNOUNCEMENT, RESERVE_FEE,
};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data));
    for flags in &[0, COND_ARGS_NIL, STRICT_ARGS_COUNT] {
        for op in &[AGG_SIG_ME, AGG_SIG_UNSAFE, ALWAYS_TRUE,
    ASSERT_COIN_ANNOUNCEMENT, ASSERT_HEIGHT_ABSOLUTE, ASSERT_HEIGHT_RELATIVE, ASSERT_MY_AMOUNT,
    ASSERT_MY_COIN_ID, ASSERT_MY_PARENT_ID, ASSERT_MY_PUZZLEHASH, ASSERT_PUZZLE_ANNOUNCEMENT,
    ASSERT_SECONDS_ABSOLUTE, ASSERT_SECONDS_RELATIVE, CREATE_COIN, CREATE_COIN_ANNOUNCEMENT,
    CREATE_PUZZLE_ANNOUNCEMENT, RESERVE_FEE] {
            let _ret = parse_args(&a, input, *op, *flags);
        }
    }
});

