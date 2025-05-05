use chia_bls::{sign, SecretKey, Signature};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
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
// use chia_protocol::Coin;
use clvmr::{
    allocator::{Allocator, NodePtr},
    reduction::EvalErr,
};
struct ConditionTest {
    opcode: ConditionOpcode,
    args: Vec<NodePtr>,
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
    condition: &ConditionTest,
) -> Result<NodePtr, EvalErr> {
    let mut rest = allocator.nil();
    for arg in condition.args.iter().rev() {
        rest = allocator.new_pair(*arg, rest)?;
    }
    let opcode = allocator.new_small_number(condition.opcode as u32)?;
    let cond_list = allocator.new_pair(opcode, rest)?;
    allocator.new_pair(cond_list, allocator.nil())
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

    // this is the list of conditions to test
    let cond_tests = [
        ConditionTest {
            opcode: opcodes::CREATE_COIN,
            args: vec![allocator.new_atom(H1).expect("atom"), one],
            aggregate_signature: Signature::default(),
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_UNSAFE,
            args: vec![
                allocator.new_atom(&pk.to_bytes()).expect("pubkey"),
                allocator.new_atom(MSG1).expect("msg"),
            ],
            aggregate_signature: sign(&sk, MSG1),
        },
    ];

    let parent_id = allocator.new_atom(H1).expect("atom");
    let puzzle_hash = Bytes32::from(clvm_utils::tree_hash_from_bytes(&[1_u8]).expect("tree_hash"));
    let puz_hash_node_ptr = allocator.new_atom(puzzle_hash.as_slice()).expect("bytes");
    // let coin = Coin {
    //     parent_coin_info: H1.into(),
    //     puzzle_hash,
    //     amount: 100,
    // };
    let cp = allocator.checkpoint();
    let mut slopes = Vec::<f64>::new();
    for cond in cond_tests {
        // let mut spend = SpendConditions::new(
        //     parent_id,
        //     100_u64,
        //     puz_hash_node_ptr,
        //     Arc::new(coin.coin_id()),
        // );

        // Create the conditions
        let conditions = create_conditions(&mut allocator, &cond).expect("create_conditions");
        // a "spend" is the following list (parent puzhash amount conditions)
        let spend_list = [parent_id, puz_hash_node_ptr, hundred, conditions];
        let mut spends = allocator.nil();
        for arg in spend_list.iter().rev() {
            spends = allocator.new_pair(*arg, spends).expect("new_pair");
        }
        let mut cost = TEST_CONSTANTS.max_block_cost_clvm;
        let mut samples = Vec::<(f64, f64)>::new();
        // Parse the conditions and then make the list longer
        for i in 0..1000 {
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
            validate_signature(&state, &cond.aggregate_signature, flags, None)
                .expect("validate_signature");

            let elapsed = start.elapsed();
            samples.push((i as f64, elapsed.as_nanos() as f64));
            // add costs to tally
            total_cost += ret.execution_cost;
            total_count += 1;

            // make the conditions list longer
            cons_condition(&mut allocator, conditions).expect("cons_condition");
        }
        // reset allocator before next condition test
        let (slope, _): (f64, f64) = linear_regression_of(&samples).expect("linreg failed");
        slopes.push(slope);
        allocator.restore_checkpoint(&cp);
    }

    println!("Slopes: {slopes:?}");
    println!("Total Cost: {total_cost}");
    println!("Total Count: {total_count}");
}
