use std::sync::Arc;

use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::gen::conditions::parse_conditions;
use chia_consensus::gen::conditions::{MempoolVisitor, SpendBundleConditions};
use chia_consensus::gen::flags::COST_CONDITIONS; // DONT_VALIDATE_SIGNATURE, NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT,
use chia_consensus::gen::opcodes;
use chia_consensus::r#gen::conditions::{ParseState, SpendConditions};
use chia_consensus::r#gen::opcodes::ConditionOpcode;
use chia_consensus::r#gen::spend_visitor::SpendVisitor;
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use clvmr::{
    allocator::{Allocator, NodePtr},
    reduction::EvalErr,
};
struct ConditionTest {
    opcode: ConditionOpcode,
    args: Vec<NodePtr>,
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

const PUBKEY: &[u8; 48] = &hex!("aefe1789d6476f60439e1168f588ea16652dc321279f05a805fbc63933e88ae9c175d6c6ab182e54af562e1a0dce41bb");

// const SECRET_KEY: &[u8; 32] =
//     &hex!("6fc9d9a2b05fd1f0e51bc91041a03be8657081f272ec281aff731624f0d1c220");

const MSG1: &[u8; 13] = &[3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3];

// const MSG2: &[u8; 19] = &[4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4];

// this function takes a NodePtr of (q . ((CONDITION ARG ARG)...))
// and add another (CONDITION ARG ARG) to the list
// fn cons_condition(allocator: &mut Allocator, current_ptr: NodePtr) -> NodePtr {}

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
    let q = allocator.new_small_number(1)?;
    let cond_list = allocator.new_pair(cond_list, allocator.nil())?;
    allocator.new_pair(q, cond_list)
}

pub fn main() {
    let mut allocator = Allocator::new();
    let mut total_cost = 0;
    let mut total_count = 0;
    // let puzzle = allocator.new_small_number(1).expect("number");
    let flags: u32 = COST_CONDITIONS;
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();

    let cond_tests = [
        ConditionTest {
            opcode: opcodes::AGG_SIG_UNSAFE,
            args: vec![
                allocator.new_atom(PUBKEY).expect("pubkey"),
                allocator.new_atom(MSG1).expect("msg"),
            ],
        },
        ConditionTest {
            opcode: opcodes::CREATE_COIN,
            args: vec![
                allocator.new_atom(H1).expect("atom"),
                allocator.new_small_number(1).expect("number"),
            ],
        },
    ];
    for cond in cond_tests.iter() {
        let parent_id = allocator.new_atom(H1).expect("atom");
        let puzzle_hash =
            Bytes32::from(clvm_utils::tree_hash_from_bytes(&[1_u8]).expect("tree_hash"));
        let puz_hash_node_ptr = allocator.new_atom(puzzle_hash.as_slice()).expect("bytes");
        let coin = Coin {
            parent_coin_info: H1.into(),
            puzzle_hash: puzzle_hash,
            amount: 1,
        };
        let mut spend = SpendConditions::new(
            parent_id,
            1_u64,
            puz_hash_node_ptr,
            Arc::new(coin.coin_id()),
        );
        let mut v = MempoolVisitor::new_spend(&mut spend);

        // Create the conditions
        let conditions = create_conditions(&mut allocator, &cond).expect("create_conditions");

        let mut cost = TEST_CONSTANTS.max_block_cost_clvm.clone();
        // Parse the conditions
        parse_conditions(
            &allocator,
            &mut ret,
            &mut state,
            spend,
            conditions,
            flags,
            &mut cost,
            &TEST_CONSTANTS,
            &mut v,
        )
        .expect("parse_conditions");

        total_cost += ret.execution_cost;
        total_count += 1;
    }

    println!("Total Cost: {}", total_cost);
    println!("Total Count: {}", total_count);
}
