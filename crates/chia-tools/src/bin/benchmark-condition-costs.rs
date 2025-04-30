use crate::gen::flags::{
    COST_CONDITIONS, DONT_VALIDATE_SIGNATURE, NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT,
};
use chia_consensus::r#gen::conditions::ParseState;
use chia_consensus::gen::conditions::{Condition, SpendBundleConditions};
use chia_consensus::gen::opcodes;
use chia_consensus::r#gen::spend_visitor::SpendVisitor;
use chia_consensus::gen::{conditions::parse_conditions, flags::COST_CONDITIONS};
use chia_consensus::r#gen::opcodes::ConditionOpcode;
use clvmr::{
    allocator::{Allocator, NodePtr},
    reduction::EvalErr,
};

struct ConditionTest {
    opcode: ConditionOpcode,
    args: Vec<NodePtr>,
}

// this function takes a NodePtr of (q . ((CONDITION ARG ARG)...))
// and add another (CONDITION ARG ARG) to the list
fn cons_condition(allocator: &mut Allocator, current_ptr: NodePtr) -> NodePtr {}

// this function generates (q . ((CONDITION ARG ARG)))
fn create_conditions(
    allocator: &mut Allocator,
    condition: ConditionTest,
) -> Result<NodePtr, EvalErr> {
    let mut rest = allocator.nil();
    for arg in condition.args.iter().rev() {
        rest = allocator.new_pair(*arg, rest)?;
    }
    let opcode = allocator.new_small_number(condition.opcode as u32)?;
    let cond_list = allocator.new_pair(opcode, rest)?;
    let q = allocator.new_small_number(1)?;
    let cond_list = allocator.new_pair(cond_list, allocator.nil())?;
    allocator.new_pair(
        q,
        cond_list,
    )
}

pub fn main() {
    let allocator = Allocator::new();
    let mut total_cost = 0;
    let mut total_count = 0;
    let puzzle = allocator.new_small_number(1).expect("number");
    let flags: u32 = COST_CONDITIONS;
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();

    let cond_tests = [
        ConditionTest {
            opcode: opcodes::AGG_SIG_UNSAFE,
            args: vec![allocator.new_small_number(1).expect("number")],
        },
        ConditionTest {
            opcode: opcodes::CREATE_COIN,
            args: vec![allocator.new_small_number(1).expect("number")],
        },
    ];

    parse_conditions::<SpendVisitor>(
        &allocator,
        &mut ret,
        &mut state,
        spend,
        conditions,
        flags,
        max_cost,
        constants,
        &mut visitor,
    );

    println!("Total Cost: {}", total_cost);
    println!("Total Count: {}", total_count);
}
