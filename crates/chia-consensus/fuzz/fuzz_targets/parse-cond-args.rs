#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::gen::conditions::parse_args;
use clvmr::allocator::Allocator;
use fuzzing_utils::{make_list, BitCursor};

use chia_consensus::gen::flags::{
    COND_ARGS_NIL, ENABLE_MESSAGE_CONDITIONS, ENABLE_SOFTFORK_CONDITION, STRICT_ARGS_COUNT,
};

use chia_consensus::gen::opcodes::{
    AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT, AGG_SIG_PARENT_PUZZLE,
    AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT, AGG_SIG_UNSAFE, ASSERT_COIN_ANNOUNCEMENT,
    ASSERT_EPHEMERAL, ASSERT_HEIGHT_ABSOLUTE, ASSERT_HEIGHT_RELATIVE, ASSERT_MY_AMOUNT,
    ASSERT_MY_COIN_ID, ASSERT_MY_PARENT_ID, ASSERT_MY_PUZZLEHASH, ASSERT_PUZZLE_ANNOUNCEMENT,
    ASSERT_SECONDS_ABSOLUTE, ASSERT_SECONDS_RELATIVE, CREATE_COIN, CREATE_COIN_ANNOUNCEMENT,
    CREATE_PUZZLE_ANNOUNCEMENT, RECEIVE_MESSAGE, REMARK, RESERVE_FEE, SEND_MESSAGE,
};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_list(&mut a, &mut BitCursor::new(data));

    for flags in &[
        0,
        COND_ARGS_NIL,
        STRICT_ARGS_COUNT,
        ENABLE_SOFTFORK_CONDITION,
        ENABLE_MESSAGE_CONDITIONS,
    ] {
        for op in &[
            AGG_SIG_ME,
            AGG_SIG_UNSAFE,
            REMARK,
            ASSERT_COIN_ANNOUNCEMENT,
            ASSERT_HEIGHT_ABSOLUTE,
            ASSERT_HEIGHT_RELATIVE,
            ASSERT_MY_AMOUNT,
            ASSERT_MY_COIN_ID,
            ASSERT_MY_PARENT_ID,
            ASSERT_MY_PUZZLEHASH,
            ASSERT_PUZZLE_ANNOUNCEMENT,
            ASSERT_SECONDS_ABSOLUTE,
            ASSERT_SECONDS_RELATIVE,
            CREATE_COIN,
            CREATE_COIN_ANNOUNCEMENT,
            CREATE_PUZZLE_ANNOUNCEMENT,
            RESERVE_FEE,
            SEND_MESSAGE,
            RECEIVE_MESSAGE,
            ASSERT_EPHEMERAL,
            AGG_SIG_PARENT,
            AGG_SIG_PUZZLE,
            AGG_SIG_AMOUNT,
            AGG_SIG_PUZZLE_AMOUNT,
            AGG_SIG_PARENT_AMOUNT,
            AGG_SIG_PARENT_PUZZLE,
        ] {
            let _ret = parse_args(&a, input, *op, *flags);
        }
    }
});
