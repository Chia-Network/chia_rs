use chia_consensus::conditions::Condition;
use chia_puzzle_types::Proof;
use chia_puzzle_types::cat::{CatArgs, CatSolution};
use chia_puzzle_types::did::{DidArgs, DidSolution};
use chia_puzzle_types::singleton::{SingletonArgs, SingletonSolution};
use chia_puzzle_types::standard::{StandardArgs, StandardSolution};
use chia_puzzles::CAT_PUZZLE_HASH;
use chia_puzzles::DID_INNERPUZ_HASH;
use chia_puzzles::P2_DELEGATED_PUZZLE_OR_HIDDEN_PUZZLE_HASH;
use chia_puzzles::SINGLETON_TOP_LAYER_V1_1_HASH;
use chia_traits::Streamable;
use clap::Parser;
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::CurriedProgram;
use clvm_utils::tree_hash;
use clvmr::{Allocator, allocator::NodePtr};

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
            Self::ReserveFee(amount) => format!("RESERVE_FEE {amount}"),
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
            Self::SendMessage(src, dst, msg) => {
                format!("SEND_MESSAGE {src:?} {dst:?} {}", msg.debug_print(a))
            }
            Self::ReceiveMessage(src, dst, msg) => {
                format!("RECEIVE_MESSAGE {src:?} {dst:?} {}", msg.debug_print(a))
            }
            Self::Skip => "[Skip] REMARK ...".to_string(),
            Self::SkipRelativeCondition => "[SkipRelativeCondition]".to_string(),
        }
    }
}

fn print_puzzle_info(a: &Allocator, puzzle: NodePtr, solution: NodePtr) {
    println!("Puzzle: {}", hex::encode(tree_hash(a, puzzle)));
    // exit if this puzzle is not curried
    let Ok(uncurried) = CurriedProgram::<NodePtr, NodePtr>::from_clvm(a, puzzle) else {
        println!("   puzzle has no curried parameters");
        return;
    };

    match tree_hash(a, uncurried.program).to_bytes() {
        P2_DELEGATED_PUZZLE_OR_HIDDEN_PUZZLE_HASH => {
            println!("p2_delegated_puzzle_or_hidden_puzzle.clsp");
            let Ok(uncurried) = CurriedProgram::<NodePtr, StandardArgs>::from_clvm(a, puzzle)
            else {
                println!("-- failed to uncurry standard transaction");
                return;
            };
            println!("    synthetic-key: {:?}", uncurried.args.synthetic_key);
            let Ok(sol) = StandardSolution::<NodePtr, NodePtr>::from_clvm(a, solution) else {
                println!("-- failed to parse solution");
                return;
            };
            println!("  solution");
            println!("    original-public-key: {:?}", sol.original_public_key);
            println!("\nDelegated Puzzle\n");
            print_puzzle_info(a, sol.delegated_puzzle, sol.solution);
        }
        CAT_PUZZLE_HASH => {
            println!("cat_v2.clsp");
            let Ok(uncurried) = CurriedProgram::<NodePtr, CatArgs<NodePtr>>::from_clvm(a, puzzle)
            else {
                println!("-- failed to uncurry CAT transaction");
                return;
            };
            println!("    mod-hash: {:?}", uncurried.args.mod_hash);
            println!("    asset-id: {:?}", uncurried.args.asset_id);
            let Ok(sol) = CatSolution::<NodePtr>::from_clvm(a, solution) else {
                println!("-- failed to parse solution");
                return;
            };

            println!("  solution");
            println!("    lineage-proof: {:?}", sol.lineage_proof);
            println!("    prev-coin-id: {:?}", sol.prev_coin_id);
            println!("    this-coin-info: {:?}", sol.this_coin_info);
            println!("    next-coin-proof: {:?}", sol.next_coin_proof);
            println!("    prev-subtotal: {:?}", sol.prev_subtotal);
            println!("    extra-delta: {:?}", sol.extra_delta);

            println!("\nInner Puzzle\n");
            print_puzzle_info(a, uncurried.args.inner_puzzle, sol.inner_puzzle_solution);
        }
        DID_INNERPUZ_HASH => {
            println!("did_innerpuz.clsp");
            let Ok(uncurried) =
                CurriedProgram::<NodePtr, DidArgs<NodePtr, NodePtr>>::from_clvm(a, puzzle)
            else {
                println!("-- failed to uncurry DID transaction");
                return;
            };
            println!(
                "    recovery_did_list_hash: {:?}",
                uncurried.args.recovery_list_hash
            );
            println!(
                "    num_verifications_required: {:?}",
                uncurried.args.num_verifications_required
            );
            println!(
                "    singleton_struct: {:?}",
                uncurried.args.singleton_struct
            );
            println!("    metadata: {:?}", uncurried.args.metadata);
            let Ok(sol) = DidSolution::<NodePtr>::from_clvm(a, solution) else {
                println!("-- failed to parse solution");
                return;
            };

            println!("\nInner Puzzle\n");
            let DidSolution::Spend(inner_sol) = sol else {
                unimplemented!();
            };
            print_puzzle_info(a, uncurried.args.inner_puzzle, inner_sol);
        }
        SINGLETON_TOP_LAYER_V1_1_HASH => {
            println!("singleton_top_layer_1_1.clsp");
            let Ok(uncurried) =
                CurriedProgram::<NodePtr, SingletonArgs<NodePtr>>::from_clvm(a, puzzle)
            else {
                println!("-- failed to uncurry singleton");
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

            let Ok(sol) = SingletonSolution::<NodePtr>::from_clvm(a, solution) else {
                println!("-- failed to parse solution");
                return;
            };
            println!("  solution");
            match sol.lineage_proof {
                Proof::Lineage(lp) => {
                    println!("    lineage-proof: {lp:?}");
                }
                Proof::Eve(ep) => {
                    println!("    eve-proof: {ep:?}");
                }
            }
            println!("    amount: {}", sol.amount);

            println!("\nInner Puzzle:\n");
            print_puzzle_info(a, uncurried.args.inner_puzzle, sol.inner_solution);
        }
        // TODO: NFT puzzles

        // Unknown puzzle
        n => {
            println!("  Unknown puzzle {}", &hex::encode(n));
        }
    }
}
fn main() {
    use chia_consensus::conditions::parse_args;
    use chia_consensus::opcodes::parse_opcode;
    use chia_consensus::error_code::{first, rest};
    use chia_protocol::CoinSpend;
    use clvmr::reduction::Reduction;
    use clvmr::{ChiaDialect, run_program};
    use std::fs::read;

    let args = Args::parse();

    let mut a = Allocator::new();
    let spend = read(args.spend).expect("spend file not found");
    let spend = CoinSpend::from_bytes(&spend).expect("parse CoinSpend");

    let puzzle = spend
        .puzzle_reveal
        .to_clvm(&mut a)
        .expect("deserialize puzzle");
    let solution = spend
        .solution
        .to_clvm(&mut a)
        .expect("deserialize solution");

    println!("Spending {:?}", &spend.coin);
    println!("   coin-id: {}\n", hex::encode(spend.coin.coin_id()));
    let dialect = ChiaDialect::new(0);
    let Reduction(_clvm_cost, conditions) =
        match run_program(&mut a, &dialect, puzzle, solution, 11_000_000_000) {
            Ok(r) => r,
            Err(eval_err) => {
                eprintln!("Error running program: {eval_err}");
                return;
            }
        };

    println!("Conditions\n");
    let mut iter = conditions;

    while let Some((mut c, next)) = a.next(iter) {
        iter = next;
        let op_ptr = first(&a, c).expect("parsing conditions");
        let Some(op) = parse_opcode(&a, op_ptr, 0) else {
            println!("  UNKNOWN CONDITION [{}]", &hex::encode(a.atom(op_ptr)));
            continue;
        };

        c = rest(&a, c).expect("parsing conditions");

        let condition = parse_args(&a, c, op, 0).expect("parse condition args");
        println!("  [{op:?}] {}", condition.debug_print(&a));
    }

    // look for known puzzles to display more information

    println!("\nPuzzle Info\n");
    print_puzzle_info(&a, puzzle, solution);
}
