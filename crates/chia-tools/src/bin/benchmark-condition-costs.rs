use chia_bls::{sign, SecretKey, Signature};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::r#gen::make_aggsig_final_message::u64_to_bytes;
use linreg::linear_regression_of;
use std::time::Instant;
use chia_sha2::Sha256;
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
    // 0 means we want to estimate a reasonable cost
    cost: u64,
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
    let Some((cond, _rest)) = allocator.next(current_ptr) else {
        return Err(EvalErr(current_ptr, "not a pair".into()));
    };
    allocator.new_pair(cond, current_ptr)
}

fn cons_two_conditions(
    allocator: &mut Allocator,
    current_ptr: NodePtr,
) -> Result<NodePtr, EvalErr> {
    let Some((cond_one, rest)) = allocator.next(current_ptr) else {
        return Err(EvalErr(current_ptr, "not a pair".into()));
    };
    let Some((cond_two, _rest)) = allocator.next(rest) else {
        return Err(EvalErr(current_ptr, "not a pair".into()));
    };
    let temp = allocator.new_pair(cond_one, current_ptr)?;
    allocator.new_pair(cond_two, temp)
}

// this function generates (q . ((CONDITION ARG ARG)))
fn create_conditions(
    allocator: &mut Allocator,
    condition: &ConditionTest<'_>,
) -> Result<NodePtr, EvalErr> {
    let mut rest = allocator.nil();
    for arg in condition.args.iter().rev() {
        rest = allocator.new_pair(*arg, rest)?;
    }
    let opcode = allocator.new_small_number(condition.opcode as u32)?;
    let cond_list = allocator.new_pair(opcode, rest)?;
    allocator.new_pair(cond_list, allocator.nil())
}

fn create_two_conditions(
    allocator: &mut Allocator,
    cond_one: &ConditionTest<'_>,
    cond_two: &ConditionTest<'_>,
) -> Result<NodePtr, EvalErr> {
    let temp = create_conditions(allocator, cond_one).expect("create_conditions");
    let mut rest = allocator.nil();
    for arg in cond_two.args.iter().rev() {
        rest = allocator.new_pair(*arg, rest).expect("create_conditions");
    }
    let opcode = allocator
        .new_small_number(cond_two.opcode as u32)
        .expect("create_conditions");
    let cond_list = allocator.new_pair(opcode, rest).expect("create_conditions");
    allocator.new_pair(cond_list, temp)
}

pub fn main() {
    let mut allocator = Allocator::new();
    let mut total_cost = 0;
    let mut total_count = 0;
    // let puzzle = allocator.new_small_number(1).expect("number");
    let flags: u32 = COST_CONDITIONS;
    let one = allocator.new_small_number(1).expect("number");
    let hundred = allocator.new_small_number(100).expect("number");
    let sixty_three = allocator.new_small_number(63).expect("number");
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
    let coin_id = allocator.new_atom(coin.coin_id().as_slice()).expect("atom");
    let h1_pointer = allocator.new_atom(H1).expect("atom");
    let pk_ptr = allocator.new_atom(&pk.to_bytes()).expect("pubkey");
    let msg_ptr = allocator.new_atom(MSG1).expect("msg");

    let mut hasher = Sha256::new();
    hasher.update([coin.coin_id().as_slice(), MSG1].concat());
    let coin_announce_msg: [u8; 32] = hasher.finalize();
    hasher = Sha256::new();
    hasher.update([puzzle_hash.as_slice(), MSG1].concat());
    let puzzle_announce_msg: [u8; 32] = hasher.finalize();

    // this is the list of conditions to test
    let cond_tests = [
        ConditionTest {
            opcode: opcodes::AGG_SIG_UNSAFE,
            args: &[pk_ptr, msg_ptr],
            aggregate_signature: sign(&sk, MSG1),
            cost: 1_200_000,
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
            cost: 1_200_000,
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
            cost: 1_200_000,
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
            cost: 1_200_000,
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
            cost: 1_200_000,
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
            cost: 1_200_000,
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
            cost: 1_200_000,
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
            cost: 1_200_000,
        },
        ConditionTest {
            opcode: opcodes::REMARK,
            args: &[h1_pointer],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        //        ConditionTest {
        //            opcode: opcodes::RESERVE_FEE,
        //            args: &[hundred],
        //            aggregate_signature: Signature::default(),
        //            cost: 0,
        //        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_COIN_ID,
            args: &[coin_id],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_PARENT_ID,
            args: &[h1_pointer],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_PUZZLEHASH,
            args: &[puz_hash_node_ptr],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_AMOUNT,
            args: &[hundred],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_BIRTH_HEIGHT,
            args: &[hundred],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_BIRTH_SECONDS,
            args: &[hundred],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        // ConditionTest {
        //     opcode: opcodes::ASSERT_EPHEMERAL,
        //     args: &[],
        //     aggregate_signature: Signature::default(),
        //     cost: 0,
        // },
        ConditionTest {
            opcode: opcodes::ASSERT_SECONDS_RELATIVE,
            args: &[one],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_SECONDS_ABSOLUTE,
            args: &[one],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_HEIGHT_RELATIVE,
            args: &[one],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_HEIGHT_ABSOLUTE,
            args: &[one],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_SECONDS_RELATIVE,
            args: &[one],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_SECONDS_ABSOLUTE,
            args: &[one],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_HEIGHT_RELATIVE,
            args: &[one],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_HEIGHT_ABSOLUTE,
            args: &[one],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::SOFTFORK,
            args: &[hundred, h1_pointer],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::SEND_MESSAGE,
            args: &[sixty_three, h1_pointer, coin_id],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_COIN_ANNOUNCEMENT,
            args: &[allocator.new_atom(&coin_announce_msg).expect("msg")],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
        ConditionTest {
            opcode: opcodes::ASSERT_PUZZLE_ANNOUNCEMENT,
            args: &[allocator.new_atom(&puzzle_announce_msg).expect("msg")],
            aggregate_signature: Signature::default(),
            cost: 0,
        },
    ];

    let receive_message = ConditionTest {
        opcode: opcodes::RECEIVE_MESSAGE,
        args: &[sixty_three, h1_pointer, coin_id],
        aggregate_signature: Signature::default(),
        cost: 0,
    };

    let coin_announcement = ConditionTest {
        opcode: opcodes::CREATE_COIN_ANNOUNCEMENT,
        args: &[msg_ptr],
        aggregate_signature: Signature::default(),
        cost: 0,
    };
    let puzzle_announcement = ConditionTest {
        opcode: opcodes::CREATE_PUZZLE_ANNOUNCEMENT,
        args: &[msg_ptr],
        aggregate_signature: Signature::default(),
        cost: 0,
    };

    let mut cost_factors = Vec::<f64>::new();
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

        let mut conditions = match cond.opcode {
            opcodes::SEND_MESSAGE => {
                create_two_conditions(&mut allocator, &cond, &receive_message).expect("two set")
            }
            opcodes::ASSERT_PUZZLE_ANNOUNCEMENT => {
                create_two_conditions(&mut allocator, &cond, &puzzle_announcement).expect("two set")
            }
            opcodes::ASSERT_COIN_ANNOUNCEMENT => {
                create_two_conditions(&mut allocator, &cond, &coin_announcement).expect("two set")
            }
            _ => create_conditions(&mut allocator, &cond).expect("create_conditions"),
        };

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
            conditions = if matches!(
                cond.opcode,
                opcodes::SEND_MESSAGE
                    | opcodes::ASSERT_PUZZLE_ANNOUNCEMENT
                    | opcodes::ASSERT_COIN_ANNOUNCEMENT
            ) {
                cons_two_conditions(&mut allocator, conditions).expect("cons_condition")
            } else {
                cons_condition(&mut allocator, conditions).expect("cons_condition")
            };
        }
        // reset allocator before next condition test
        let (slope, _): (f64, f64) = linear_regression_of(&samples).expect("linreg failed");
        if cond.cost > 0 {
            let cost_per_ns = cond.cost as f64 / slope;
            cost_factors.push(cost_per_ns);
            println!(
                "condition: {} slope: {slope} cost-per-nanosecond: {cost_per_ns}",
                cond.opcode
            );
        } else {
            let cost_per_ns = cost_factors.iter().sum::<f64>() / cost_factors.len() as f64;
            if matches!(
                cond.opcode,
                opcodes::SEND_MESSAGE
                    | opcodes::ASSERT_PUZZLE_ANNOUNCEMENT
                    | opcodes::ASSERT_COIN_ANNOUNCEMENT
            ) {
                println!(
                    "condition: {} slope: {slope} computed-cost: {}",
                    cond.opcode,
                    (slope * cost_per_ns) / 2 as f64
                );
            } else {
                println!(
                    "condition: {} slope: {slope} computed-cost: {}",
                    cond.opcode,
                    slope * cost_per_ns
                );
            }
        };
        allocator.restore_checkpoint(&cp);
    }

    println!("Total Cost: {total_cost}");
    println!("Total Count: {total_count}");
}
