use chia_bls::{sign, SecretKey, Signature};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::r#gen::make_aggsig_final_message::u64_to_bytes;
use linreg::linear_regression_of;
use std::time::Instant;
// use chia_consensus::gen::conditions::parse_conditions;
use chia_consensus::gen::conditions::{MempoolVisitor, SpendBundleConditions};
use chia_consensus::gen::flags::COST_CONDITIONS; // DONT_VALIDATE_SIGNATURE, NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT,
use chia_consensus::gen::opcodes;
use chia_consensus::r#gen::conditions::{
    process_single_spend,
    validate_conditions,
    validate_signature,
    ParseState, // SpendConditions,
};
use chia_consensus::r#gen::opcodes::ConditionOpcode;
use chia_consensus::r#gen::spend_visitor::SpendVisitor;
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use clvmr::{
    allocator::{Allocator, NodePtr},
    reduction::EvalErr,
};
struct ConditionTest<'a> {
    opcode: ConditionOpcode,
    args: &'a [NodePtr],
    aggregate_signature: Signature,
}
use hex_literal::hex;

const H1: &[u8; 32] = &[
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

// const H2: &[u8; 32] = &[
//     2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
// ];

// const LONG_VEC: &[u8; 33] = &[
//     3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
//     3,
// ];

// const PUBKEY: &[u8; 48] = &hex!("aefe1789d6476f60439e1168f588ea16652dc321279f05a805fbc63933e88ae9c175d6c6ab182e54af562e1a0dce41bb");

const SECRET_KEY: &[u8; 32] =
    &hex!("6fc9d9a2b05fd1f0e51bc91041a03be8657081f272ec281aff731624f0d1c220");

const MSG1: &[u8; 13] = &[3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3];

// const MSG2: &[u8; 19] = &[4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4];

// this function takes a NodePtr of (q . ((CONDITION ARG ARG)...))
// and add another (CONDITION ARG ARG) to the list
fn cons_condition(allocator: &mut Allocator, current_ptr: NodePtr) -> Result<NodePtr, EvalErr> {
    // let Some((q, conds)) = allocator.next(current_ptr) else {
    //     return Err(EvalErr(current_ptr, "not a pair".into()));
    // };
    let Some((cond, _rest)) = allocator.next(current_ptr) else {
        return Err(EvalErr(current_ptr, "not a pair".into()));
    };
    allocator.new_pair(cond, current_ptr)
    // allocator.new_pair(q, added_new_cond)
}

// this function generates (q . ((CONDITION ARG ARG)))
fn create_conditions(
    allocator: &mut Allocator,
    condition: &ConditionTest<'_>,
    reps: u32,
) -> Result<NodePtr, EvalErr> {
    let mut rest = allocator.nil();
    for arg in condition.args.iter().rev() {
        rest = allocator.new_pair(*arg, rest)?;
    }
    let opcode = allocator.new_small_number(condition.opcode as u32)?;
    let cond_list = allocator.new_pair(opcode, rest)?;

    let mut ret = NodePtr::NIL;
    for _ in 0..reps {
        ret = allocator.new_pair(cond_list, ret)?;
    }
    Ok(ret)
}

pub fn main() {
    let mut allocator = Allocator::new();
    let mut total_cost = 0;
    let mut total_count = 0;
    // let puzzle = allocator.new_small_number(1).expect("number");
    let flags: u32 = COST_CONDITIONS;
    let one = allocator.new_small_number(1).expect("number");
    let hundred = allocator.new_small_number(100).expect("number");
    let sk = SecretKey::from_bytes(SECRET_KEY).expect("secret key");
    let pk = sk.public_key();
    let parent_id = allocator.new_atom(H1).expect("atom");
    let puzzle_hash = Bytes32::from(clvm_utils::tree_hash_from_bytes(&[1_u8]).expect("tree_hash"));
    let puz_hash_node_ptr = allocator.new_atom(puzzle_hash.as_slice()).expect("bytes");
    let coin = Coin {
        parent_coin_info: H1.into(),
        puzzle_hash,
        amount: 100,
    };
    let h1_pointer = allocator.new_atom(H1).expect("atom");
    let pk_ptr = allocator.new_atom(&pk.to_bytes()).expect("pubkey");
    let msg_ptr = allocator.new_atom(MSG1).expect("msg");

    // this is the list of conditions to test
    let cond_tests = [
        ConditionTest {
            opcode: opcodes::REMARK,
            args: &[h1_pointer],
            aggregate_signature: Signature::default(),
        },
        //        ConditionTest {
        //            opcode: opcodes::CREATE_COIN,
        //            args: vec![h1_pointer, one],
        //            aggregate_signature: Signature::default(),
        //        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_UNSAFE,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(&sk, MSG1),
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_ME,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(
                &sk,
                [
                    MSG1,
                    coin.coin_id().as_slice(),
                    TEST_CONSTANTS.agg_sig_me_additional_data.as_slice(),
                ]
                .concat(),
            ),
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PARENT,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(
                &sk,
                [
                    MSG1,
                    H1.as_slice(),
                    TEST_CONSTANTS.agg_sig_parent_additional_data.as_slice(),
                ]
                .concat(),
            ),
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PUZZLE,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(
                &sk,
                [
                    MSG1,
                    puzzle_hash.as_slice(),
                    TEST_CONSTANTS.agg_sig_puzzle_additional_data.as_slice(),
                ]
                .concat(),
            ),
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_AMOUNT,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(
                &sk,
                [
                    MSG1,
                    u64_to_bytes(100_u64).as_slice(),
                    TEST_CONSTANTS.agg_sig_amount_additional_data.as_slice(),
                ]
                .concat(),
            ),
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PARENT_AMOUNT,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(
                &sk,
                [
                    MSG1,
                    H1.as_slice(),
                    u64_to_bytes(100_u64).as_slice(),
                    TEST_CONSTANTS
                        .agg_sig_parent_amount_additional_data
                        .as_slice(),
                ]
                .concat(),
            ),
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PARENT_PUZZLE,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(
                &sk,
                [
                    MSG1,
                    H1.as_slice(),
                    puzzle_hash.as_slice(),
                    TEST_CONSTANTS
                        .agg_sig_parent_puzzle_additional_data
                        .as_slice(),
                ]
                .concat(),
            ),
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PUZZLE_AMOUNT,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(
                &sk,
                [
                    MSG1,
                    puzzle_hash.as_slice(),
                    u64_to_bytes(100_u64).as_slice(),
                    TEST_CONSTANTS
                        .agg_sig_puzzle_amount_additional_data
                        .as_slice(),
                ]
                .concat(),
            ),
        },
        //        ConditionTest {
        //            opcode: opcodes::RESERVE_FEE,
        //            args: &[hundred],
        //            aggregate_signature: Signature::default(),
        //        },
        ConditionTest {
            opcode: opcodes::CREATE_COIN_ANNOUNCEMENT,
            args: &[h1_pointer],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::CREATE_PUZZLE_ANNOUNCEMENT,
            args: &[h1_pointer],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_COIN_ID,
            args: &[allocator.new_atom(coin.coin_id().as_slice()).expect("atom")],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_PARENT_ID,
            args: &[h1_pointer],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_PUZZLEHASH,
            args: &[puz_hash_node_ptr],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_AMOUNT,
            args: &[hundred],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_BIRTH_HEIGHT,
            args: &[hundred],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_BIRTH_SECONDS,
            args: &[hundred],
            aggregate_signature: Signature::default(),
        },
        // ConditionTest {
        //     opcode: opcodes::ASSERT_EPHEMERAL,
        //     args: &[],
        //     aggregate_signature: Signature::default(),
        // },
        ConditionTest {
            opcode: opcodes::ASSERT_SECONDS_RELATIVE,
            args: &[one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_SECONDS_ABSOLUTE,
            args: &[one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_HEIGHT_RELATIVE,
            args: &[one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_HEIGHT_ABSOLUTE,
            args: &[one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_SECONDS_RELATIVE,
            args: &[one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_SECONDS_ABSOLUTE,
            args: &[one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_HEIGHT_RELATIVE,
            args: &[one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_HEIGHT_ABSOLUTE,
            args: &[one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::SOFTFORK,
            args: &[hundred, h1_pointer],
            aggregate_signature: Signature::default(),
        },
    ];

    let mut slopes = Vec::<f64>::new();
    for cond in cond_tests {
        // let mut spend = SpendConditions::new(
        //     parent_id,
        //     100_u64,
        //     puz_hash_node_ptr,
        //     Arc::new(coin.coin_id()),
        // );
        let mut cost = u64::MAX;
        let mut samples = Vec::<(f64, f64)>::new();
        let mut signature = Signature::default();
        let cp = allocator.checkpoint();
        // Parse the conditions and then make the list longer
        let mut conditions =
            create_conditions(&mut allocator, &cond, 1).expect("create_conditions");
        for i in 1..500 {
            signature += &cond.aggregate_signature;
            let mut spends = allocator.nil();
            // a "spend" is the following list (parent puzhash amount conditions)
            let spend_list = [parent_id, puz_hash_node_ptr, hundred, conditions];
            for arg in spend_list.iter().rev() {
                spends = allocator.new_pair(*arg, spends).expect("new_pair");
            }
            // need to reset state or we get a double spend
            let mut ret = SpendBundleConditions::default();
            let mut state = ParseState::default();

            let start = Instant::now();
            process_single_spend::<MempoolVisitor>(
                &allocator,
                &mut ret,
                &mut state,
                parent_id,
                puz_hash_node_ptr,
                hundred,
                conditions,
                flags,
                &mut cost,
                &TEST_CONSTANTS,
            )
            .expect("process_single_spend");

            MempoolVisitor::post_process(&allocator, &state, &mut ret).expect("post_process");
            validate_conditions(&allocator, &ret, &state, spends, flags)
                .expect("validate_conditions");
            validate_signature(&state, &signature, flags, None).expect("validate_signature");

            let elapsed = start.elapsed();
            // the first run is a warmup
            if i > 1 {
                samples.push((i as f64, elapsed.as_nanos() as f64));
            }
            // add costs to tally
            total_cost += ret.cost;
            total_count += 1;

            // make the conditions list longer
            conditions = cons_condition(&mut allocator, conditions).expect("cons_condition");
        }
        // reset allocator before next condition test
        let (slope, _): (f64, f64) = linear_regression_of(&samples).expect("linreg failed");
        println!("condition: {} slope: {slope}", cond.opcode);
        slopes.push(slope);
        allocator.restore_checkpoint(&cp);
    }

    println!("Slopes: {slopes:?}");
    println!("Total Cost: {total_cost}");
    println!("Total Count: {total_count}");
}
