use crate::error::{Error, Result};
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use chia_wallet::singleton::SINGLETON_TOP_LAYER_PUZZLE_HASH;
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::CurriedProgram;
use clvm_utils::{tree_hash, tree_hash_atom, tree_hash_pair};
use clvmr::allocator::{Allocator, NodePtr};

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
pub struct SingletonSolution<I> {
    pub lineage_proof: LineageProof,
    pub amount: u64,
    pub inner_solution: I,
}

// TODO: replace this with a generic function to compute the hash of curried
// puzzles
const OP_QUOTE: u8 = 1;
const OP_APPLY: u8 = 2;
const OP_CONS: u8 = 4;
fn curry_single_arg(arg_hash: [u8; 32], rest: [u8; 32]) -> [u8; 32] {
    tree_hash_pair(
        tree_hash_atom(&[OP_CONS]),
        tree_hash_pair(
            tree_hash_pair(tree_hash_atom(&[OP_QUOTE]), arg_hash),
            tree_hash_pair(rest, tree_hash_atom(&[])),
        ),
    )
}

fn curry_and_treehash(inner_puzzle_hash: &Bytes32, singleton_struct: &SingletonStruct) -> Bytes32 {
    let singleton_struct_hash = tree_hash_pair(
        tree_hash_atom(&singleton_struct.mod_hash),
        tree_hash_pair(
            tree_hash_atom(&singleton_struct.launcher_id),
            tree_hash_atom(&singleton_struct.launcher_puzzle_hash),
        ),
    );

    let args_hash = tree_hash_atom(&[OP_QUOTE]);
    let args_hash = curry_single_arg(inner_puzzle_hash.into(), args_hash);
    let args_hash = curry_single_arg(singleton_struct_hash, args_hash);

    tree_hash_pair(
        tree_hash_atom(&[OP_APPLY]),
        tree_hash_pair(
            tree_hash_pair(
                tree_hash_atom(&[OP_QUOTE]),
                (&singleton_struct.mod_hash).into(),
            ),
            tree_hash_pair(args_hash, tree_hash_atom(&[])),
        ),
    )
    .into()
}

// given a puzzle, solution and new coin of a singleton
// this function validates the lineage proof and returns a new
// solution spending a new coin ID.
// The existing coin to be spent and the new coin's parent must also be passed in
// for validation.
pub fn fast_forward_singleton(
    a: &mut Allocator,
    puzzle: NodePtr,
    solution: NodePtr,
    coin: &Coin,       // the current coin being spent (for validation)
    new_coin: &Coin,   // the new coin to spend
    new_parent: &Coin, // the parent coin of the new coin being spent
) -> Result<NodePtr> {
    // a coin with an even amount is not a valid singleton
    // as defined by singleton_top_layer_v1_1.clsp
    if (coin.amount & 1) == 0 || (new_parent.amount & 1) == 0 || (new_coin.amount & 1) == 0 {
        return Err(Error::CoinAmountEven);
    }

    // we can only fast-forward spends of singletons whose puzzle hash doesn't
    // change
    if coin.puzzle_hash != new_parent.puzzle_hash || coin.puzzle_hash != new_coin.puzzle_hash {
        return Err(Error::PuzzleHashMismatch);
    }

    let singleton = CurriedProgram::<NodePtr, SingletonArgs<NodePtr>>::from_clvm(a, puzzle)?;
    let mut new_solution = SingletonSolution::<NodePtr>::from_clvm(a, solution)?;

    // this is the tree hash of the singleton top layer puzzle
    // the tree hash of singleton_top_layer_v1_1.clsp
    if singleton.args.singleton_struct.mod_hash.as_ref() != SINGLETON_TOP_LAYER_PUZZLE_HASH {
        return Err(Error::NotSingletonModHash);
    }

    // also make sure the actual mod-hash of this puzzle matches the
    // singleton_top_layer_v1_1.clsp
    let mod_hash = tree_hash(a, singleton.program);
    if mod_hash != SINGLETON_TOP_LAYER_PUZZLE_HASH {
        return Err(Error::NotSingletonModHash);
    }

    // if the current solution to the puzzle doesn't match the coin amount, it's
    // an invalid spend. Don't try to fast-forward it
    if coin.amount != new_solution.amount {
        return Err(Error::CoinAmountMismatch);
    }

    // given the parent's parent, the parent's inner puzzle and parent's amount,
    // we can compute the hash of the curried inner puzzle for our parent coin
    let parent_puzzle_hash = curry_and_treehash(
        &new_solution.lineage_proof.parent_inner_puzzle_hash,
        &singleton.args.singleton_struct,
    );

    // now that we know the parent coin's puzzle hash, we have all the pieces to
    // compute the coin being spent (before the fast-forward).
    let parent_coin = Coin {
        parent_coin_info: new_solution.lineage_proof.parent_parent_coin_id,
        puzzle_hash: parent_puzzle_hash,
        amount: new_solution.lineage_proof.parent_amount,
    };

    if parent_coin.coin_id() != *coin.parent_coin_info {
        return Err(Error::ParentCoinMismatch);
    }

    let inner_puzzle_hash = tree_hash(a, singleton.args.inner_puzzle);
    if inner_puzzle_hash != *new_solution.lineage_proof.parent_inner_puzzle_hash {
        return Err(Error::InnerPuzzleHashMismatch);
    }

    let puzzle_hash = tree_hash(a, puzzle);

    if puzzle_hash != *new_parent.puzzle_hash || puzzle_hash != *coin.puzzle_hash {
        // we can only fast-forward if the puzzle hash match the new coin
        // the spend is assumed to be valied already, so we don't check it
        // against the original coin being spent
        return Err(Error::PuzzleHashMismatch);
    }

    // update the solution to use the new parent coin's information
    new_solution.lineage_proof.parent_parent_coin_id = new_parent.parent_coin_info;
    new_solution.lineage_proof.parent_amount = new_parent.amount;
    new_solution.amount = new_coin.amount;

    let expected_new_parent = new_parent.coin_id();

    if *new_coin.parent_coin_info != expected_new_parent {
        return Err(Error::CoinMismatch);
    }

    Ok(new_solution.to_clvm(a)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::conditions::MempoolVisitor;
    use crate::gen::run_puzzle::run_puzzle;
    use chia_protocol::CoinSpend;
    use chia_traits::streamable::Streamable;
    use clvm_traits::ToNodePtr;
    use clvmr::serde::{node_from_bytes, node_to_bytes};
    use hex_literal::hex;
    use rstest::rstest;
    use std::fs;

    // this test loads CoinSpends from file (Coin, puzzle, solution)-triples
    // and "fast-forwards" the spend onto a few different parent-parent coins
    // and ensures the spends are still valid
    #[rstest]
    #[case("e3c0")]
    #[case("bb13")]
    fn test_fast_forward(
        #[case] spend_file: &str,
        #[values(
            "abababababababababababababababababababababababababababababababab",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        )]
        new_parents_parent: &str,
        #[values(0, 1, 3, 5)] new_amount: u64,
        #[values(0, 1, 3, 5)] prev_amount: u64,
    ) {
        let spend_bytes = fs::read(format!("ff-tests/{spend_file}.spend")).expect("read file");
        let spend = CoinSpend::from_bytes(&spend_bytes).expect("parse CoinSpend");
        let new_parents_parent = hex::decode(new_parents_parent).unwrap();

        let mut a = Allocator::new_limited(500000000);
        let puzzle = spend.puzzle_reveal.to_node_ptr(&mut a).expect("to_clvm");
        let solution = spend.solution.to_node_ptr(&mut a).expect("to_clvm");
        let puzzle_hash = Bytes32::from(tree_hash(&a, puzzle));

        let new_parent_coin = Coin {
            parent_coin_info: new_parents_parent.try_into().unwrap(),
            puzzle_hash,
            amount: if prev_amount == 0 {
                spend.coin.amount
            } else {
                prev_amount
            },
        };

        let new_coin = Coin {
            parent_coin_info: new_parent_coin.coin_id().into(),
            puzzle_hash,
            amount: if new_amount == 0 {
                spend.coin.amount
            } else {
                new_amount
            },
        };

        // perform fast-forward
        let new_solution = fast_forward_singleton(
            &mut a,
            puzzle,
            solution,
            &spend.coin,
            &new_coin,
            &new_parent_coin,
        )
        .expect("fast-forward");
        let new_solution = node_to_bytes(&a, new_solution).expect("serialize new solution");

        // run original spend
        let conditions1 = run_puzzle::<MempoolVisitor>(
            &mut a,
            spend.puzzle_reveal.as_slice(),
            spend.solution.as_slice(),
            &spend.coin.parent_coin_info,
            spend.coin.amount,
            11000000000,
            0,
        )
        .expect("run_puzzle");

        // run new spend
        let conditions2 = run_puzzle::<MempoolVisitor>(
            &mut a,
            spend.puzzle_reveal.as_slice(),
            new_solution.as_slice(),
            &new_coin.parent_coin_info,
            new_coin.amount,
            11000000000,
            0,
        )
        .expect("run_puzzle");

        assert!(conditions1.spends[0].create_coin == conditions2.spends[0].create_coin);
    }

    fn run_ff_test(
        mutate: fn(&mut Allocator, &mut Coin, &mut Coin, &mut Coin, &mut Vec<u8>, &mut Vec<u8>),
        expected_err: Error,
    ) {
        let spend_bytes = fs::read("ff-tests/e3c0.spend").expect("read file");
        let mut spend = CoinSpend::from_bytes(&spend_bytes).expect("parse CoinSpend");
        let new_parents_parent: &[u8] =
            &hex!("abababababababababababababababababababababababababababababababab");

        let mut a = Allocator::new_limited(500000000);
        let puzzle = spend.puzzle_reveal.to_node_ptr(&mut a).expect("to_clvm");
        let puzzle_hash = Bytes32::from(tree_hash(&a, puzzle));

        let mut new_parent_coin = Coin {
            parent_coin_info: new_parents_parent.try_into().unwrap(),
            puzzle_hash,
            amount: spend.coin.amount,
        };

        let mut new_coin = Coin {
            parent_coin_info: new_parent_coin.coin_id().into(),
            puzzle_hash,
            amount: spend.coin.amount,
        };

        let mut puzzle = spend.puzzle_reveal.as_slice().to_vec();
        let mut solution = spend.solution.as_slice().to_vec();
        mutate(
            &mut a,
            &mut spend.coin,
            &mut new_coin,
            &mut new_parent_coin,
            &mut puzzle,
            &mut solution,
        );

        let puzzle = node_from_bytes(&mut a, puzzle.as_slice()).expect("to_clvm");
        let solution = node_from_bytes(&mut a, solution.as_slice()).expect("to_clvm");

        // attempt fast-forward
        assert_eq!(
            fast_forward_singleton(
                &mut a,
                puzzle,
                solution,
                &spend.coin,
                &new_coin,
                &new_parent_coin
            )
            .unwrap_err(),
            expected_err
        );
    }

    #[test]
    fn test_even_amount() {
        run_ff_test(
            |_a, coin, _new_coin, _new_parent, _puzzle, _solution| {
                coin.amount = 2;
            },
            Error::CoinAmountEven,
        );

        run_ff_test(
            |_a, _coin, new_coin, _new_parent, _puzzle, _solution| {
                new_coin.amount = 2;
            },
            Error::CoinAmountEven,
        );

        run_ff_test(
            |_a, _coin, _new_coin, new_parent, _puzzle, _solution| {
                new_parent.amount = 2;
            },
            Error::CoinAmountEven,
        );
    }

    #[test]
    fn test_amount_mismatch() {
        run_ff_test(
            |_a, coin, _new_coin, _new_parent, _puzzle, _solution| {
                coin.amount = 3;
            },
            Error::CoinAmountMismatch,
        );
    }

    fn parse_solution(a: &mut Allocator, solution: &[u8]) -> SingletonSolution<NodePtr> {
        let new_solution = node_from_bytes(a, solution).expect("parse solution");
        SingletonSolution::from_clvm(a, new_solution).expect("parse solution")
    }

    fn serialize_solution(a: &mut Allocator, solution: &SingletonSolution<NodePtr>) -> Vec<u8> {
        let new_solution = solution.to_clvm(a).expect("to_clvm");
        node_to_bytes(a, new_solution).expect("serialize solution")
    }

    fn parse_singleton(
        a: &mut Allocator,
        puzzle: &[u8],
    ) -> CurriedProgram<NodePtr, SingletonArgs<NodePtr>> {
        let puzzle = node_from_bytes(a, puzzle).expect("parse puzzle");
        CurriedProgram::<NodePtr, SingletonArgs<NodePtr>>::from_clvm(a, puzzle).expect("uncurry")
    }

    fn serialize_singleton(
        a: &mut Allocator,
        singleton: &CurriedProgram<NodePtr, SingletonArgs<NodePtr>>,
    ) -> Vec<u8> {
        let puzzle = singleton.to_clvm(a).expect("to_clvm");
        node_to_bytes(a, puzzle).expect("serialize puzzle")
    }

    #[test]
    fn test_invalid_lineage_proof_parent() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, _puzzle, solution| {
                let mut new_solution = parse_solution(a, solution);

                // corrupt the lineage proof
                new_solution.lineage_proof.parent_parent_coin_id = Bytes32::from(hex!(
                    "fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe"
                ));

                *solution = serialize_solution(a, &new_solution);
            },
            Error::ParentCoinMismatch,
        );
    }

    #[test]
    fn test_invalid_lineage_proof_parent_amount() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, _puzzle, solution| {
                let mut new_solution = parse_solution(a, solution);

                // corrupt the lineage proof
                new_solution.lineage_proof.parent_amount = 11;

                *solution = serialize_solution(a, &new_solution);
            },
            Error::ParentCoinMismatch,
        );
    }

    #[test]
    fn test_invalid_lineage_proof_parent_inner_ph() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, _puzzle, solution| {
                let mut new_solution = parse_solution(a, solution);

                // corrupt the lineage proof
                new_solution.lineage_proof.parent_inner_puzzle_hash = Bytes32::from(hex!(
                    "fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe"
                ));

                *solution = serialize_solution(a, &new_solution);
            },
            Error::ParentCoinMismatch,
        );
    }

    #[test]
    fn test_invalid_lineage_proof_parent_inner_ph_with_coin() {
        run_ff_test(
            |a, coin, new_coin, new_parent, puzzle, solution| {
                let mut new_solution = parse_solution(a, solution);
                let singleton = parse_singleton(a, puzzle);

                // corrupt the lineage proof
                new_solution.lineage_proof.parent_inner_puzzle_hash = Bytes32::from(hex!(
                    "fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe"
                ));

                // adjust the coins puzzle hashes to match
                let parent_puzzle_hash = curry_and_treehash(
                    &new_solution.lineage_proof.parent_inner_puzzle_hash,
                    &singleton.args.singleton_struct,
                );

                *solution = serialize_solution(a, &new_solution);

                *new_parent = Coin {
                    parent_coin_info: new_solution.lineage_proof.parent_parent_coin_id,
                    puzzle_hash: parent_puzzle_hash,
                    amount: new_solution.lineage_proof.parent_amount,
                };

                new_coin.puzzle_hash = parent_puzzle_hash;

                coin.parent_coin_info = new_parent.coin_id().into();
                coin.puzzle_hash = parent_puzzle_hash;
            },
            Error::InnerPuzzleHashMismatch,
        );
    }

    #[test]
    fn test_invalid_puzzle_hash() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, puzzle, _solution| {
                let mut singleton = parse_singleton(a, puzzle);

                singleton.program = a.nil();

                *puzzle = serialize_singleton(a, &singleton);
            },
            Error::NotSingletonModHash,
        );
    }

    #[test]
    fn test_invalid_singleton_struct_puzzle_hash() {
        run_ff_test(
            |a, _coin, _new_coin, _new_parent, puzzle, _solution| {
                let mut singleton = parse_singleton(a, puzzle);

                singleton.args.singleton_struct.mod_hash = Bytes32::from(hex!(
                    "fefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefe"
                ));

                *puzzle = serialize_singleton(a, &singleton);
            },
            Error::NotSingletonModHash,
        );
    }
}
