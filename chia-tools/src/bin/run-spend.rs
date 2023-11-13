use chia::gen::conditions::Condition;
use chia_protocol::Bytes32;
use chia_traits::Streamable;
use clap::Parser;
use clvm_traits::{FromClvm, FromPtr, ToClvm};
use clvm_utils::tree_hash;
use clvm_utils::CurriedProgram;
use clvmr::serde::node_from_bytes;
use clvmr::{allocator::NodePtr, Allocator};
use hex_literal::hex;
use std::io::Cursor;

/// Run a puzzle given a solution and print the resulting conditions
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to CoinSpend (serialized binary file)
    spend: String,
}

trait DebugPrint {
    fn debug_print(&self, a: &Allocator) -> String;
}

impl DebugPrint for NodePtr {
    fn debug_print(&self, a: &Allocator) -> String {
        hex::encode(a.atom(*self))
    }
}

impl DebugPrint for Condition {
    // TODO: it would be nice if this was a macro
    fn debug_print(&self, a: &Allocator) -> String {
        match self {
            Self::AggSigUnsafe(pk, msg) => format!(
                "AGG_SIG_UNSAFE {} {}",
                pk.debug_print(a),
                msg.debug_print(a)
            ),
            Self::AggSigMe(pk, msg) => {
                format!("AGG_SIG_ME {} {}", pk.debug_print(a), msg.debug_print(a))
            }
            Self::AggSigParent(pk, msg) => format!(
                "AGG_SIG_PARENT {} {}",
                pk.debug_print(a),
                msg.debug_print(a)
            ),
            Self::AggSigPuzzle(pk, msg) => format!(
                "AGG_SIG_PUZZLE {} {}",
                pk.debug_print(a),
                msg.debug_print(a)
            ),
            Self::AggSigAmount(pk, msg) => format!(
                "AGG_SIG_AMOUNT {} {}",
                pk.debug_print(a),
                msg.debug_print(a)
            ),
            Self::AggSigPuzzleAmount(pk, msg) => format!(
                "AGG_SIG_PUZZLE_AMOUNT {} {}",
                pk.debug_print(a),
                msg.debug_print(a)
            ),
            Self::AggSigParentAmount(pk, msg) => format!(
                "AGG_SIG_PARENT_AMOUNT {} {}",
                pk.debug_print(a),
                msg.debug_print(a)
            ),
            Self::AggSigParentPuzzle(pk, msg) => format!(
                "AGG_SIG_PARENT_PUZZLE {} {}",
                pk.debug_print(a),
                msg.debug_print(a)
            ),
            Self::CreateCoin(ph, amount, hint) => format!(
                "CRATE_COIN {} {} {}",
                ph.debug_print(a),
                amount,
                hint.debug_print(a)
            ),
            Self::ReserveFee(amount) => format!("RESERVE_FEE {}", amount),
            Self::CreateCoinAnnouncement(msg) => {
                format!("CREATE_COIN_ANNOUNCEMENT {}", msg.debug_print(a))
            }
            Self::CreatePuzzleAnnouncement(msg) => {
                format!("CREATE_PUZZLE_ANNOUNCEMENT {}", msg.debug_print(a))
            }
            Self::AssertCoinAnnouncement(msg) => {
                format!("ASSERT_COIN_ANNOUNCEMENT {}", msg.debug_print(a))
            }
            Self::AssertPuzzleAnnouncement(msg) => {
                format!("ASSERT_PUZZLE_ANNOUNCEMENT {}", msg.debug_print(a))
            }
            Self::AssertConcurrentSpend(coinid) => {
                format!("ASSERT_CONCURRENT_SPEND {}", coinid.debug_print(a))
            }
            Self::AssertConcurrentPuzzle(ph) => {
                format!("ASSERT_CONCURRENT_PUZZLE {}", ph.debug_print(a))
            }
            Self::AssertMyCoinId(coinid) => format!("ASSERT_MY_COINID {}", coinid.debug_print(a)),
            Self::AssertMyParentId(coinid) => {
                format!("ASSERT_MY_PARENT_ID {}", coinid.debug_print(a))
            }
            Self::AssertMyPuzzlehash(ph) => format!("ASSERT_MY_PUZZLE_HASH {}", ph.debug_print(a)),
            Self::AssertMyAmount(amount) => format!("ASSERT_MY_AMOUNT {amount}"),
            Self::AssertMyBirthSeconds(s) => format!("ASSERT_MY_BIRTH_SECONDS {s}"),
            Self::AssertMyBirthHeight(h) => format!("ASSERT_MY_BIRTH_HEIGHT {h}"),
            Self::AssertSecondsRelative(s) => format!("ASSERT_SECONDS_RELATIVE {s}"),
            Self::AssertSecondsAbsolute(s) => format!("ASSERT_SECONDS_ABSOLUTE {s}"),
            Self::AssertHeightRelative(h) => format!("ASSERT_HEIGHT_RELATIVE {h}"),
            Self::AssertHeightAbsolute(h) => format!("ASSERT_HEIGHT_ABSOLUTE {h}"),
            Self::AssertBeforeSecondsRelative(s) => format!("ASSERT_BEFORE_SECONDS_RELATIVE {s}"),
            Self::AssertBeforeSecondsAbsolute(s) => format!("ASSERT_BEFORE_SECONDS_ABSOLUTE {s}"),
            Self::AssertBeforeHeightRelative(h) => format!("ASSERT_BEFORE_HEIGHT_RELATIVE {h}"),
            Self::AssertBeforeHeightAbsolute(h) => format!("ASSERT_BEFORE_HEIGHT_ABSOLUTE {h}"),
            Self::AssertEphemeral => "ASSERT_EPHEMERAL".to_string(),
            Self::Softfork(cost) => format!("SOFTFORK {cost}"),
            Self::Skip => "[Skip] REMARK ...".to_string(),
            Self::SkipRelativeCondition => "[SkipRelativeCondition]".to_string(),
        }
    }
}

const SINGLETON_MOD_HASH: [u8; 32] =
    hex!("7faa3253bfddd1e0decb0906b2dc6247bbc4cf608f58345d173adb63e8b47c9f");

#[derive(FromClvm, ToClvm, Debug)]
#[clvm(tuple)]
pub struct SingletonStruct {
    pub mod_hash: Bytes32,
    pub launcher_id: Bytes32,
    pub launcher_puzzle_hash: Bytes32,
}

#[derive(FromClvm, ToClvm, Debug)]
#[clvm(curry)]
pub struct SingletonArgs<I> {
    pub singleton_struct: SingletonStruct,
    pub inner_puzzle: I,
}

#[derive(FromClvm, ToClvm, Debug)]
#[clvm(list)]
pub struct LineageProof {
    pub parent_parent_coin_id: Bytes32,
    pub parent_inner_puzzle_hash: Bytes32,
    pub parent_amount: u64,
}

#[derive(FromClvm, ToClvm, Debug)]
#[clvm(list)]
pub struct EveProof {
    pub parent_parent_coin_id: Bytes32,
    pub parent_amount: u64,
}

#[derive(FromClvm, ToClvm, Debug)]
#[clvm(list)]
pub struct SingletonSolution<I> {
    pub lineage_proof: LineageProof,
    pub amount: u64,
    pub inner_solution: I,
}

#[derive(FromClvm, ToClvm, Debug)]
#[clvm(list)]
pub struct EveSingletonSolution<I> {
    pub lineage_proof: EveProof,
    pub amount: u64,
    pub inner_solution: I,
}

fn print_puzzle_info(a: &Allocator, puzzle: NodePtr, solution: NodePtr) {
    println!("Puzzle: {}", hex::encode(tree_hash(a, puzzle)));
    // exit if this puzzle is not curried
    let Ok(uncurried) = <CurriedProgram<NodePtr, NodePtr>>::from_ptr(a, puzzle) else {
        println!("   puzzle has no curried parameters");
        return;
    };

    match tree_hash(a, uncurried.program) {
        SINGLETON_MOD_HASH => {
            println!("singleton_top_layer_1_1.clsp");
            let Ok(uncurried) =
                <CurriedProgram<NodePtr, SingletonArgs<NodePtr>>>::from_ptr(a, puzzle)
            else {
                println!("failed to uncurry singleton");
                return;
            };
            println!("  singleton-struct:");
            println!(
                "    mod-hash: {:?}",
                uncurried.args.singleton_struct.mod_hash
            );
            println!(
                "    launcher-id: {:?}",
                uncurried.args.singleton_struct.launcher_id
            );
            println!(
                "    launcher-puzzle-hash: {:?}",
                uncurried.args.singleton_struct.launcher_puzzle_hash
            );

            let inner_solution =
                if let Ok(sol) = <SingletonSolution<NodePtr>>::from_ptr(a, solution) {
                    println!("  solution");
                    println!("    lineage-proof: {:?}", sol.lineage_proof);
                    println!("    amount: {}", sol.amount);
                    sol.inner_solution
                } else if let Ok(sol) = <EveSingletonSolution<NodePtr>>::from_ptr(a, solution) {
                    println!("  eve-solution:");
                    println!("    lineage-proof:: {:?}", sol.lineage_proof);
                    println!("    amount: {}", sol.amount);
                    sol.inner_solution
                } else {
                    println!("-- failed to parse singleton solution");
                    return;
                };

            println!("\nInner Puzzle:\n");
            print_puzzle_info(a, uncurried.args.inner_puzzle, inner_solution);
        }

        // Unknown puzzle
        n => {
            println!("  Unknown puzzle {}", &hex::encode(n));
        }
    }
}
fn main() {
    use chia::gen::conditions::parse_args;
    use chia::gen::flags::ENABLE_SOFTFORK_CONDITION;
    use chia::gen::opcodes::parse_opcode;
    use chia::gen::validation_error::{first, rest};
    use chia_protocol::coin_spend::CoinSpend;
    use clvmr::reduction::{EvalErr, Reduction};
    use clvmr::{run_program, ChiaDialect};
    use std::fs::read;

    let args = Args::parse();

    let mut a = Allocator::new();
    let spend = read(args.spend).expect("spend file not found");
    let spend = CoinSpend::parse(&mut Cursor::new(spend.as_slice())).expect("parse CoinSpend");

    let puzzle =
        node_from_bytes(&mut a, spend.puzzle_reveal.as_slice()).expect("deserialize puzzle");
    let solution =
        node_from_bytes(&mut a, spend.solution.as_slice()).expect("deserialize solution");

    println!("Spending {:?}", &spend.coin);
    println!("   coin-id: {}\n", hex::encode(spend.coin.coin_id()));
    let dialect = ChiaDialect::new(0);
    let Reduction(_clvm_cost, conditions) =
        match run_program(&mut a, &dialect, puzzle, solution, 11000000000) {
            Ok(r) => r,
            Err(EvalErr(_, e)) => {
                println!("Eval Error: {e:?}");
                return;
            }
        };

    println!("Conditions\n");
    let mut iter = conditions;

    while let Some((mut c, next)) = a.next(iter) {
        iter = next;
        let op_ptr = first(&a, c).expect("parsing conditions");
        let op = match parse_opcode(&a, op_ptr, ENABLE_SOFTFORK_CONDITION) {
            None => {
                println!("  UNKNOWN CONDITION [{}]", &hex::encode(a.atom(op_ptr)));
                continue;
            }
            Some(v) => v,
        };

        c = rest(&a, c).expect("parsing conditions");

        let condition = parse_args(&a, c, op, 0).expect("parse condition args");
        println!("  [{op:?}] {}", condition.debug_print(&a));
    }

    // look for known puzzles to display more information

    println!("\nPuzzle Info\n");
    print_puzzle_info(&a, puzzle, solution);
}
