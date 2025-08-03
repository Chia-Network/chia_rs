#![no_main]
use libfuzzer_sys::{arbitrary, fuzz_target};

use chia_consensus::conditions::parse_args;
use clvmr::allocator::Allocator;

use chia_consensus::flags::STRICT_ARGS_COUNT;

use chia_consensus::opcodes::{
    AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT, AGG_SIG_PARENT_PUZZLE,
    AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT, AGG_SIG_UNSAFE, ASSERT_COIN_ANNOUNCEMENT,
    ASSERT_EPHEMERAL, ASSERT_HEIGHT_ABSOLUTE, ASSERT_HEIGHT_RELATIVE, ASSERT_MY_AMOUNT,
    ASSERT_MY_COIN_ID, ASSERT_MY_PARENT_ID, ASSERT_MY_PUZZLEHASH, ASSERT_PUZZLE_ANNOUNCEMENT,
    ASSERT_SECONDS_ABSOLUTE, ASSERT_SECONDS_RELATIVE, CREATE_COIN, CREATE_COIN_ANNOUNCEMENT,
    CREATE_PUZZLE_ANNOUNCEMENT, RECEIVE_MESSAGE, REMARK, RESERVE_FEE, SEND_MESSAGE,
};
use clvm_fuzzing::make_list;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let mut unstructured = arbitrary::Unstructured::new(data);
    let input = make_list(&mut a, &mut unstructured);

    for flags in &[0, STRICT_ARGS_COUNT] {
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
