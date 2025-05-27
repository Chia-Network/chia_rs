use chia_bls::PublicKey;
use chia_protocol::{Bytes32, Coin};
use chia_puzzles::{CAT_PUZZLE_HASH, EVERYTHING_WITH_SIGNATURE_HASH, GENESIS_BY_COIN_ID_HASH};
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::{CurriedProgram, ToTreeHash, TreeHash};

use crate::{CoinProof, LineageProof};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct CatArgs<I> {
    pub mod_hash: Bytes32,
    pub asset_id: Bytes32,
    pub inner_puzzle: I,
}

impl<I> CatArgs<I> {
    pub fn new(asset_id: Bytes32, inner_puzzle: I) -> Self {
        Self {
            mod_hash: CAT_PUZZLE_HASH.into(),
            asset_id,
            inner_puzzle,
        }
    }
}

impl CatArgs<TreeHash> {
    pub fn curry_tree_hash(asset_id: Bytes32, inner_puzzle: TreeHash) -> TreeHash {
        CurriedProgram {
            program: TreeHash::new(CAT_PUZZLE_HASH),
            args: CatArgs {
                mod_hash: CAT_PUZZLE_HASH.into(),
                asset_id,
                inner_puzzle,
            },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct EverythingWithSignatureTailArgs {
    pub public_key: PublicKey,
}

impl EverythingWithSignatureTailArgs {
    pub fn new(public_key: PublicKey) -> Self {
        Self { public_key }
    }

    pub fn curry_tree_hash(public_key: PublicKey) -> TreeHash {
        CurriedProgram {
            program: TreeHash::new(EVERYTHING_WITH_SIGNATURE_HASH),
            args: EverythingWithSignatureTailArgs::new(public_key),
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct GenesisByCoinIdTailArgs {
    pub genesis_coin_id: Bytes32,
}

impl GenesisByCoinIdTailArgs {
    pub fn new(genesis_coin_id: Bytes32) -> Self {
        Self { genesis_coin_id }
    }

    pub fn curry_tree_hash(genesis_coin_id: Bytes32) -> TreeHash {
        CurriedProgram {
            program: TreeHash::new(GENESIS_BY_COIN_ID_HASH),
            args: GenesisByCoinIdTailArgs::new(genesis_coin_id),
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct CatSolution<I> {
    pub inner_puzzle_solution: I,
    pub lineage_proof: Option<LineageProof>,
    pub prev_coin_id: Bytes32,
    pub this_coin_info: Coin,
    pub next_coin_proof: CoinProof,
    pub prev_subtotal: i64,
    pub extra_delta: i64,
}

#[cfg(test)]
mod tests {
    use chia_puzzles::{
        CAT_PUZZLE, EVERYTHING_WITH_SIGNATURE, GENESIS_BY_COIN_ID,
        P2_DELEGATED_PUZZLE_OR_HIDDEN_PUZZLE,
    };
    use clvm_traits::ToClvm;
    use clvm_utils::tree_hash;
    use clvmr::{serde::node_from_bytes, Allocator};

    use super::*;

    use crate::standard::StandardArgs;

    #[test]
    fn curry_cat_tree_hash() {
        let synthetic_key = PublicKey::default();
        let asset_id = Bytes32::new([120; 32]);

        let mut a = Allocator::new();
        let mod_ptr = node_from_bytes(&mut a, &CAT_PUZZLE).unwrap();
        let inner_mod_ptr = node_from_bytes(&mut a, &P2_DELEGATED_PUZZLE_OR_HIDDEN_PUZZLE).unwrap();

        let curried_ptr = CurriedProgram {
            program: mod_ptr,
            args: CatArgs::new(
                asset_id,
                CurriedProgram {
                    program: inner_mod_ptr,
                    args: StandardArgs::new(synthetic_key),
                },
            ),
        }
        .to_clvm(&mut a)
        .unwrap();

        let allocated_tree_hash = hex::encode(tree_hash(&a, curried_ptr));

        let inner_puzzle_hash = StandardArgs::curry_tree_hash(synthetic_key);
        let tree_hash = hex::encode(CatArgs::curry_tree_hash(asset_id, inner_puzzle_hash));

        assert_eq!(allocated_tree_hash, tree_hash);
    }

    #[test]
    fn curry_everything_with_signature() {
        let public_key = PublicKey::default();

        let mut a = Allocator::new();
        let mod_ptr = node_from_bytes(&mut a, &EVERYTHING_WITH_SIGNATURE).unwrap();

        let curried_ptr = CurriedProgram {
            program: mod_ptr,
            args: EverythingWithSignatureTailArgs::new(public_key),
        }
        .to_clvm(&mut a)
        .unwrap();

        let allocated_tree_hash = hex::encode(tree_hash(&a, curried_ptr));

        let tree_hash = hex::encode(EverythingWithSignatureTailArgs::curry_tree_hash(public_key));

        assert_eq!(allocated_tree_hash, tree_hash);
    }

    #[test]
    fn curry_genesis_by_coin_id() {
        let genesis_coin_id = Bytes32::new([120; 32]);

        let mut a = Allocator::new();
        let mod_ptr = node_from_bytes(&mut a, &GENESIS_BY_COIN_ID).unwrap();

        let curried_ptr = CurriedProgram {
            program: mod_ptr,
            args: GenesisByCoinIdTailArgs::new(genesis_coin_id),
        }
        .to_clvm(&mut a)
        .unwrap();

        let allocated_tree_hash = hex::encode(tree_hash(&a, curried_ptr));

        let tree_hash = hex::encode(GenesisByCoinIdTailArgs::curry_tree_hash(genesis_coin_id));

        assert_eq!(allocated_tree_hash, tree_hash);
    }
}
