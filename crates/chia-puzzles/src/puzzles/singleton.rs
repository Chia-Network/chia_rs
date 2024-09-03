use chia_protocol::Bytes32;
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::{CurriedProgram, ToTreeHash, TreeHash};
use hex_literal::hex;

use crate::Proof;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct SingletonArgs<I> {
    pub singleton_struct: SingletonStruct,
    pub inner_puzzle: I,
}

impl<I> SingletonArgs<I> {
    pub fn new(launcher_id: Bytes32, inner_puzzle: I) -> Self {
        Self {
            singleton_struct: SingletonStruct::new(launcher_id),
            inner_puzzle,
        }
    }
}

impl SingletonArgs<TreeHash> {
    pub fn curry_tree_hash(launcher_id: Bytes32, inner_puzzle: TreeHash) -> TreeHash {
        CurriedProgram {
            program: SINGLETON_TOP_LAYER_PUZZLE_HASH,
            args: SingletonArgs {
                singleton_struct: SingletonStruct::new(launcher_id),
                inner_puzzle,
            },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct SingletonStruct {
    pub mod_hash: Bytes32,
    pub launcher_id: Bytes32,
    #[clvm(rest)]
    pub launcher_puzzle_hash: Bytes32,
}

impl SingletonStruct {
    pub fn new(launcher_id: Bytes32) -> Self {
        Self {
            mod_hash: SINGLETON_TOP_LAYER_PUZZLE_HASH.into(),
            launcher_id,
            launcher_puzzle_hash: SINGLETON_LAUNCHER_PUZZLE_HASH.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(solution)]
pub struct SingletonSolution<I> {
    pub lineage_proof: Proof,
    pub amount: u64,
    pub inner_solution: I,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(solution)]
pub struct LauncherSolution<T> {
    pub singleton_puzzle_hash: Bytes32,
    pub amount: u64,
    pub key_value_list: T,
}

/// This is the puzzle reveal of the [singleton launcher](https://chialisp.com/singletons#launcher) puzzle.
pub const SINGLETON_LAUNCHER_PUZZLE: [u8; 175] = hex!(
    "
    ff02ffff01ff04ffff04ff04ffff04ff05ffff04ff0bff80808080ffff04ffff
    04ff0affff04ffff02ff0effff04ff02ffff04ffff04ff05ffff04ff0bffff04
    ff17ff80808080ff80808080ff808080ff808080ffff04ffff01ff33ff3cff02
    ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff0effff04ff02ffff04ff
    09ff80808080ffff02ff0effff04ff02ffff04ff0dff8080808080ffff01ff0b
    ffff0101ff058080ff0180ff018080
    "
);

/// This is the puzzle hash of the [singleton launcher](https://chialisp.com/singletons#launcher) puzzle.
pub const SINGLETON_LAUNCHER_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    eff07522495060c066f66f32acc2a77e3a3e737aca8baea4d1a64ea4cdc13da9
    "
));

/// This is the puzzle reveal of the [singleton](https://chialisp.com/singletons) puzzle.
pub const SINGLETON_TOP_LAYER_PUZZLE: [u8; 967] = hex!(
    "
    ff02ffff01ff02ffff03ffff18ff2fff3480ffff01ff04ffff04ff20ffff04ff
    2fff808080ffff04ffff02ff3effff04ff02ffff04ff05ffff04ffff02ff2aff
    ff04ff02ffff04ff27ffff04ffff02ffff03ff77ffff01ff02ff36ffff04ff02
    ffff04ff09ffff04ff57ffff04ffff02ff2effff04ff02ffff04ff05ff808080
    80ff808080808080ffff011d80ff0180ffff04ffff02ffff03ff77ffff0181b7
    ffff015780ff0180ff808080808080ffff04ff77ff808080808080ffff02ff3a
    ffff04ff02ffff04ff05ffff04ffff02ff0bff5f80ffff01ff80808080808080
    80ffff01ff088080ff0180ffff04ffff01ffffffff4947ff0233ffff0401ff01
    02ffffff20ff02ffff03ff05ffff01ff02ff32ffff04ff02ffff04ff0dffff04
    ffff0bff3cffff0bff34ff2480ffff0bff3cffff0bff3cffff0bff34ff2c80ff
    0980ffff0bff3cff0bffff0bff34ff8080808080ff8080808080ffff010b80ff
    0180ffff02ffff03ffff22ffff09ffff0dff0580ff2280ffff09ffff0dff0b80
    ff2280ffff15ff17ffff0181ff8080ffff01ff0bff05ff0bff1780ffff01ff08
    8080ff0180ff02ffff03ff0bffff01ff02ffff03ffff02ff26ffff04ff02ffff
    04ff13ff80808080ffff01ff02ffff03ffff20ff1780ffff01ff02ffff03ffff
    09ff81b3ffff01818f80ffff01ff02ff3affff04ff02ffff04ff05ffff04ff1b
    ffff04ff34ff808080808080ffff01ff04ffff04ff23ffff04ffff02ff36ffff
    04ff02ffff04ff09ffff04ff53ffff04ffff02ff2effff04ff02ffff04ff05ff
    80808080ff808080808080ff738080ffff02ff3affff04ff02ffff04ff05ffff
    04ff1bffff04ff34ff8080808080808080ff0180ffff01ff088080ff0180ffff
    01ff04ff13ffff02ff3affff04ff02ffff04ff05ffff04ff1bffff04ff17ff80
    80808080808080ff0180ffff01ff02ffff03ff17ff80ffff01ff088080ff0180
    80ff0180ffffff02ffff03ffff09ff09ff3880ffff01ff02ffff03ffff18ff2d
    ffff010180ffff01ff0101ff8080ff0180ff8080ff0180ff0bff3cffff0bff34
    ff2880ffff0bff3cffff0bff3cffff0bff34ff2c80ff0580ffff0bff3cffff02
    ff32ffff04ff02ffff04ff07ffff04ffff0bff34ff3480ff8080808080ffff0b
    ff34ff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02
    ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ffff04ff0d
    ff8080808080ffff01ff0bffff0101ff058080ff0180ff02ffff03ffff21ff17
    ffff09ff0bff158080ffff01ff04ff30ffff04ff0bff808080ffff01ff088080
    ff0180ff018080
    "
);

/// This is the puzzle hash of the [singleton](https://chialisp.com/singletons) puzzle.
pub const SINGLETON_TOP_LAYER_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    7faa3253bfddd1e0decb0906b2dc6247bbc4cf608f58345d173adb63e8b47c9f
    "
));

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_puzzle_hash;

    #[test]
    fn puzzle_hashes() {
        assert_puzzle_hash!(SINGLETON_LAUNCHER_PUZZLE => SINGLETON_LAUNCHER_PUZZLE_HASH);
        assert_puzzle_hash!(SINGLETON_TOP_LAYER_PUZZLE => SINGLETON_TOP_LAYER_PUZZLE_HASH);
    }
}
