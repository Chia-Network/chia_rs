use chia_bls::PublicKey;
use chia_protocol::{Bytes32, Coin};
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::{CurriedProgram, ToTreeHash, TreeHash};
use hex_literal::hex;

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
            program: CAT_PUZZLE_HASH,
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
            program: EVERYTHING_WITH_SIGNATURE_TAIL_PUZZLE_HASH,
            args: EverythingWithSignatureTailArgs { public_key },
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
            program: GENESIS_BY_COIN_ID_TAIL_PUZZLE_HASH,
            args: GenesisByCoinIdTailArgs { genesis_coin_id },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(solution)]
pub struct CatSolution<I> {
    pub inner_puzzle_solution: I,
    pub lineage_proof: Option<LineageProof>,
    pub prev_coin_id: Bytes32,
    pub this_coin_info: Coin,
    pub next_coin_proof: CoinProof,
    pub prev_subtotal: i64,
    pub extra_delta: i64,
}

/// This is the puzzle reveal of the [CAT2 standard](https://chialisp.com/cats) puzzle.
pub const CAT_PUZZLE: [u8; 1672] = hex!(
    "
    ff02ffff01ff02ff5effff04ff02ffff04ffff04ff05ffff04ffff0bff34ff05
    80ffff04ff0bff80808080ffff04ffff02ff17ff2f80ffff04ff5fffff04ffff
    02ff2effff04ff02ffff04ff17ff80808080ffff04ffff02ff2affff04ff02ff
    ff04ff82027fffff04ff82057fffff04ff820b7fff808080808080ffff04ff81
    bfffff04ff82017fffff04ff8202ffffff04ff8205ffffff04ff820bffff8080
    8080808080808080808080ffff04ffff01ffffffff3d46ff02ff333cffff0401
    ff01ff81cb02ffffff20ff02ffff03ff05ffff01ff02ff32ffff04ff02ffff04
    ff0dffff04ffff0bff7cffff0bff34ff2480ffff0bff7cffff0bff7cffff0bff
    34ff2c80ff0980ffff0bff7cff0bffff0bff34ff8080808080ff8080808080ff
    ff010b80ff0180ffff02ffff03ffff22ffff09ffff0dff0580ff2280ffff09ff
    ff0dff0b80ff2280ffff15ff17ffff0181ff8080ffff01ff0bff05ff0bff1780
    ffff01ff088080ff0180ffff02ffff03ff0bffff01ff02ffff03ffff09ffff02
    ff2effff04ff02ffff04ff13ff80808080ff820b9f80ffff01ff02ff56ffff04
    ff02ffff04ffff02ff13ffff04ff5fffff04ff17ffff04ff2fffff04ff81bfff
    ff04ff82017fffff04ff1bff8080808080808080ffff04ff82017fff80808080
    80ffff01ff088080ff0180ffff01ff02ffff03ff17ffff01ff02ffff03ffff20
    ff81bf80ffff0182017fffff01ff088080ff0180ffff01ff088080ff018080ff
    0180ff04ffff04ff05ff2780ffff04ffff10ff0bff5780ff778080ffffff02ff
    ff03ff05ffff01ff02ffff03ffff09ffff02ffff03ffff09ff11ff5880ffff01
    59ff8080ff0180ffff01818f80ffff01ff02ff26ffff04ff02ffff04ff0dffff
    04ff0bffff04ffff04ff81b9ff82017980ff808080808080ffff01ff02ff7aff
    ff04ff02ffff04ffff02ffff03ffff09ff11ff5880ffff01ff04ff58ffff04ff
    ff02ff76ffff04ff02ffff04ff13ffff04ff29ffff04ffff0bff34ff5b80ffff
    04ff2bff80808080808080ff398080ffff01ff02ffff03ffff09ff11ff7880ff
    ff01ff02ffff03ffff20ffff02ffff03ffff09ffff0121ffff0dff298080ffff
    01ff02ffff03ffff09ffff0cff29ff80ff3480ff5c80ffff01ff0101ff8080ff
    0180ff8080ff018080ffff0109ffff01ff088080ff0180ffff010980ff018080
    ff0180ffff04ffff02ffff03ffff09ff11ff5880ffff0159ff8080ff0180ffff
    04ffff02ff26ffff04ff02ffff04ff0dffff04ff0bffff04ff17ff8080808080
    80ff80808080808080ff0180ffff01ff04ff80ffff04ff80ff17808080ff0180
    ffff02ffff03ff05ffff01ff04ff09ffff02ff56ffff04ff02ffff04ff0dffff
    04ff0bff808080808080ffff010b80ff0180ff0bff7cffff0bff34ff2880ffff
    0bff7cffff0bff7cffff0bff34ff2c80ff0580ffff0bff7cffff02ff32ffff04
    ff02ffff04ff07ffff04ffff0bff34ff3480ff8080808080ffff0bff34ff8080
    808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff2effff04
    ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff0dff80808080
    80ffff01ff0bffff0101ff058080ff0180ffff04ffff04ff30ffff04ff5fff80
    8080ffff02ff7effff04ff02ffff04ffff04ffff04ff2fff0580ffff04ff5fff
    82017f8080ffff04ffff02ff26ffff04ff02ffff04ff0bffff04ff05ffff01ff
    808080808080ffff04ff17ffff04ff81bfffff04ff82017fffff04ffff02ff2a
    ffff04ff02ffff04ff8204ffffff04ffff02ff76ffff04ff02ffff04ff09ffff
    04ff820affffff04ffff0bff34ff2d80ffff04ff15ff80808080808080ffff04
    ff8216ffff808080808080ffff04ff8205ffffff04ff820bffff808080808080
    808080808080ff02ff5affff04ff02ffff04ff5fffff04ff3bffff04ffff02ff
    ff03ff17ffff01ff09ff2dffff02ff2affff04ff02ffff04ff27ffff04ffff02
    ff76ffff04ff02ffff04ff29ffff04ff57ffff04ffff0bff34ff81b980ffff04
    ff59ff80808080808080ffff04ff81b7ff80808080808080ff8080ff0180ffff
    04ff17ffff04ff05ffff04ff8202ffffff04ffff04ffff04ff78ffff04ffff0e
    ff5cffff02ff2effff04ff02ffff04ffff04ff2fffff04ff82017fff808080ff
    8080808080ff808080ffff04ffff04ff20ffff04ffff0bff81bfff5cffff02ff
    2effff04ff02ffff04ffff04ff15ffff04ffff10ff82017fffff11ff8202dfff
    2b80ff8202ff80ff808080ff8080808080ff808080ff138080ff808080808080
    80808080ff018080
    "
);

/// This is the puzzle hash of the [CAT2 standard](https://chialisp.com/cats) puzzle.
pub const CAT_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    37bef360ee858133b69d595a906dc45d01af50379dad515eb9518abb7c1d2a7a
    "
));

/// This is the puzzle reveal of the [CAT2 multi-issuance TAIL](https://chialisp.com/cats#multi) puzzle.
pub const EVERYTHING_WITH_SIGNATURE_TAIL_PUZZLE: [u8; 41] = hex!(
    "
    ff02ffff01ff04ffff04ff02ffff04ff05ffff04ff5fff80808080ff8080ffff
    04ffff0132ff018080
    "
);

/// This is the puzzle hash of the [CAT2 multi-issuance TAIL](https://chialisp.com/cats#multi) puzzle.
pub const EVERYTHING_WITH_SIGNATURE_TAIL_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    1720d13250a7c16988eaf530331cefa9dd57a76b2c82236bec8bbbff91499b89
    "
));

/// This is the puzzle reveal of the [CAT2 single-issuance TAIL](https://chialisp.com/cats/#single) puzzle.
pub const GENESIS_BY_COIN_ID_TAIL_PUZZLE: [u8; 45] = hex!(
    "
    ff02ffff03ff2fffff01ff0880ffff01ff02ffff03ffff09ff2dff0280ff80ff
    ff01ff088080ff018080ff0180
    "
);

/// This is the puzzle hash of the [CAT2 single-issuance TAIL](https://chialisp.com/cats/#single) puzzle.
pub const GENESIS_BY_COIN_ID_TAIL_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    493afb89eed93ab86741b2aa61b8f5de495d33ff9b781dfc8919e602b2afa150
    "
));

/// This is the puzzle reveal of the old [CAT1 standard](https://chialisp.com/cats) puzzle.
///
/// **Warning:**
/// It is recommended not to use CAT1 for anything other than backwards compatibility (e.g. offer compression),
/// due to security issues uncovered in an audit. You can read more about the vulnerability that prompted the creation
/// of CAT2 in the [CATbleed Post Mortem](https://github.com/Chia-Network/post-mortem/blob/main/2022-08/2022-08-19-CATbleed.md).
pub const CAT_PUZZLE_V1: [u8; 1420] = hex!(
    "
    ff02ffff01ff02ff5effff04ff02ffff04ffff04ff05ffff04ffff0bff2cff05
    80ffff04ff0bff80808080ffff04ffff02ff17ff2f80ffff04ff5fffff04ffff
    02ff2effff04ff02ffff04ff17ff80808080ffff04ffff0bff82027fff82057f
    ff820b7f80ffff04ff81bfffff04ff82017fffff04ff8202ffffff04ff8205ff
    ffff04ff820bffff80808080808080808080808080ffff04ffff01ffffffff81
    ca3dff46ff0233ffff3c04ff01ff0181cbffffff02ff02ffff03ff05ffff01ff
    02ff32ffff04ff02ffff04ff0dffff04ffff0bff22ffff0bff2cff3480ffff0b
    ff22ffff0bff22ffff0bff2cff5c80ff0980ffff0bff22ff0bffff0bff2cff80
    80808080ff8080808080ffff010b80ff0180ffff02ffff03ff0bffff01ff02ff
    ff03ffff09ffff02ff2effff04ff02ffff04ff13ff80808080ff820b9f80ffff
    01ff02ff26ffff04ff02ffff04ffff02ff13ffff04ff5fffff04ff17ffff04ff
    2fffff04ff81bfffff04ff82017fffff04ff1bff8080808080808080ffff04ff
    82017fff8080808080ffff01ff088080ff0180ffff01ff02ffff03ff17ffff01
    ff02ffff03ffff20ff81bf80ffff0182017fffff01ff088080ff0180ffff01ff
    088080ff018080ff0180ffff04ffff04ff05ff2780ffff04ffff10ff0bff5780
    ff778080ff02ffff03ff05ffff01ff02ffff03ffff09ffff02ffff03ffff09ff
    11ff7880ffff0159ff8080ff0180ffff01818f80ffff01ff02ff7affff04ff02
    ffff04ff0dffff04ff0bffff04ffff04ff81b9ff82017980ff808080808080ff
    ff01ff02ff5affff04ff02ffff04ffff02ffff03ffff09ff11ff7880ffff01ff
    04ff78ffff04ffff02ff36ffff04ff02ffff04ff13ffff04ff29ffff04ffff0b
    ff2cff5b80ffff04ff2bff80808080808080ff398080ffff01ff02ffff03ffff
    09ff11ff2480ffff01ff04ff24ffff04ffff0bff20ff2980ff398080ffff0109
    80ff018080ff0180ffff04ffff02ffff03ffff09ff11ff7880ffff0159ff8080
    ff0180ffff04ffff02ff7affff04ff02ffff04ff0dffff04ff0bffff04ff17ff
    808080808080ff80808080808080ff0180ffff01ff04ff80ffff04ff80ff1780
    8080ff0180ffffff02ffff03ff05ffff01ff04ff09ffff02ff26ffff04ff02ff
    ff04ff0dffff04ff0bff808080808080ffff010b80ff0180ff0bff22ffff0bff
    2cff5880ffff0bff22ffff0bff22ffff0bff2cff5c80ff0580ffff0bff22ffff
    02ff32ffff04ff02ffff04ff07ffff04ffff0bff2cff2c80ff8080808080ffff
    0bff2cff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff
    02ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff
    0dff8080808080ffff01ff0bff2cff058080ff0180ffff04ffff04ff28ffff04
    ff5fff808080ffff02ff7effff04ff02ffff04ffff04ffff04ff2fff0580ffff
    04ff5fff82017f8080ffff04ffff02ff7affff04ff02ffff04ff0bffff04ff05
    ffff01ff808080808080ffff04ff17ffff04ff81bfffff04ff82017fffff04ff
    ff0bff8204ffffff02ff36ffff04ff02ffff04ff09ffff04ff820affffff04ff
    ff0bff2cff2d80ffff04ff15ff80808080808080ff8216ff80ffff04ff8205ff
    ffff04ff820bffff808080808080808080808080ff02ff2affff04ff02ffff04
    ff5fffff04ff3bffff04ffff02ffff03ff17ffff01ff09ff2dffff0bff27ffff
    02ff36ffff04ff02ffff04ff29ffff04ff57ffff04ffff0bff2cff81b980ffff
    04ff59ff80808080808080ff81b78080ff8080ff0180ffff04ff17ffff04ff05
    ffff04ff8202ffffff04ffff04ffff04ff24ffff04ffff0bff7cff2fff82017f
    80ff808080ffff04ffff04ff30ffff04ffff0bff81bfffff0bff7cff15ffff10
    ff82017fffff11ff8202dfff2b80ff8202ff808080ff808080ff138080ff8080
    8080808080808080ff018080
    "
);

/// This is the puzzle hash of the old [CAT1 standard](https://chialisp.com/cats) puzzle.
///
/// **Warning:**
/// It is recommended not to use CAT1 for anything other than backwards compatibility (e.g. offer compression),
/// due to security issues uncovered in an audit. You can read more about the vulnerability that prompted the creation
/// of CAT2 in the [CATbleed Post Mortem](https://github.com/Chia-Network/post-mortem/blob/main/2022-08/2022-08-19-CATbleed.md).
pub const CAT_PUZZLE_HASH_V1: TreeHash = TreeHash::new(hex!(
    "
    72dec062874cd4d3aab892a0906688a1ae412b0109982e1797a170add88bdcdc
    "
));

#[cfg(test)]
mod tests {
    use clvm_traits::ToClvm;
    use clvm_utils::tree_hash;
    use clvmr::{serde::node_from_bytes, Allocator};

    use super::*;

    use crate::{
        assert_puzzle_hash,
        standard::{StandardArgs, STANDARD_PUZZLE},
    };

    #[test]
    fn puzzle_hashes() {
        assert_puzzle_hash!(CAT_PUZZLE => CAT_PUZZLE_HASH);
        assert_puzzle_hash!(CAT_PUZZLE_V1 => CAT_PUZZLE_HASH_V1);
        assert_puzzle_hash!(EVERYTHING_WITH_SIGNATURE_TAIL_PUZZLE => EVERYTHING_WITH_SIGNATURE_TAIL_PUZZLE_HASH);
        assert_puzzle_hash!(GENESIS_BY_COIN_ID_TAIL_PUZZLE => GENESIS_BY_COIN_ID_TAIL_PUZZLE_HASH);
    }

    #[test]
    fn curry_cat_tree_hash() {
        let synthetic_key = PublicKey::default();
        let asset_id = Bytes32::new([120; 32]);

        let mut a = Allocator::new();
        let mod_ptr = node_from_bytes(&mut a, &CAT_PUZZLE).unwrap();
        let inner_mod_ptr = node_from_bytes(&mut a, &STANDARD_PUZZLE).unwrap();

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
        let mod_ptr = node_from_bytes(&mut a, &EVERYTHING_WITH_SIGNATURE_TAIL_PUZZLE).unwrap();

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
        let mod_ptr = node_from_bytes(&mut a, &GENESIS_BY_COIN_ID_TAIL_PUZZLE).unwrap();

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
