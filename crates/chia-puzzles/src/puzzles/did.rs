use chia_bls::PublicKey;
use chia_protocol::Bytes32;
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::{CurriedProgram, ToTreeHash, TreeHash};
use hex_literal::hex;

use crate::{singleton::SingletonStruct, CoinProof};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct DidArgs<I, M> {
    pub inner_puzzle: I,
    pub recovery_list_hash: Option<Bytes32>,
    pub num_verifications_required: u64,
    pub singleton_struct: SingletonStruct,
    pub metadata: M,
}

impl<I, M> DidArgs<I, M> {
    pub fn new(
        inner_puzzle: I,
        recovery_list_hash: Option<Bytes32>,
        num_verifications_required: u64,
        singleton_struct: SingletonStruct,
        metadata: M,
    ) -> Self {
        Self {
            inner_puzzle,
            recovery_list_hash,
            num_verifications_required,
            singleton_struct,
            metadata,
        }
    }
}

impl DidArgs<TreeHash, TreeHash> {
    pub fn curry_tree_hash(
        inner_puzzle: TreeHash,
        recovery_list_hash: Option<Bytes32>,
        num_verifications_required: u64,
        singleton_struct: SingletonStruct,
        metadata: TreeHash,
    ) -> TreeHash {
        CurriedProgram {
            program: DID_INNER_PUZZLE_HASH,
            args: DidArgs {
                inner_puzzle,
                recovery_list_hash,
                num_verifications_required,
                singleton_struct,
                metadata,
            },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(solution)]
#[repr(u8)]
pub enum DidSolution<I> {
    Recover(#[clvm(rest)] Box<DidRecoverySolution>) = 0,
    Spend(I) = 1,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(solution)]
pub struct DidRecoverySolution {
    pub amount: u64,
    pub new_inner_puzzle_hash: Bytes32,
    pub recovery_coins: Vec<CoinProof>,
    pub public_key: PublicKey,
    pub recovery_list_reveal: Vec<Bytes32>,
}

/// This is the puzzle reveal of the [DID1 standard](https://chialisp.com/dids) puzzle.
pub const DID_INNER_PUZZLE: [u8; 1012] = hex!(
    "
    ff02ffff01ff02ffff03ff81bfffff01ff02ff05ff82017f80ffff01ff02ffff
    03ffff22ffff09ffff02ff7effff04ff02ffff04ff8217ffff80808080ff0b80
    ffff15ff17ff808080ffff01ff04ffff04ff28ffff04ff82017fff808080ffff
    04ffff04ff34ffff04ff8202ffffff04ff82017fffff04ffff04ff8202ffff80
    80ff8080808080ffff04ffff04ff38ffff04ff822fffff808080ffff02ff26ff
    ff04ff02ffff04ff2fffff04ff17ffff04ff8217ffffff04ff822fffffff04ff
    8202ffffff04ff8205ffffff04ff820bffffff01ff8080808080808080808080
    808080ffff01ff088080ff018080ff0180ffff04ffff01ffffffff313dff4946
    ffff0233ff3c04ffffff0101ff02ff02ffff03ff05ffff01ff02ff3affff04ff
    02ffff04ff0dffff04ffff0bff2affff0bff22ff3c80ffff0bff2affff0bff2a
    ffff0bff22ff3280ff0980ffff0bff2aff0bffff0bff22ff8080808080ff8080
    808080ffff010b80ff0180ffffff02ffff03ff17ffff01ff02ffff03ff82013f
    ffff01ff04ffff04ff30ffff04ffff0bffff0bffff02ff36ffff04ff02ffff04
    ff05ffff04ff27ffff04ff82023fffff04ff82053fffff04ff820b3fff808080
    8080808080ffff02ff7effff04ff02ffff04ffff02ff2effff04ff02ffff04ff
    2fffff04ff5fffff04ff82017fff808080808080ff8080808080ff2f80ff8080
    80ffff02ff26ffff04ff02ffff04ff05ffff04ff0bffff04ff37ffff04ff2fff
    ff04ff5fffff04ff8201bfffff04ff82017fffff04ffff10ff8202ffffff0101
    80ff808080808080808080808080ffff01ff02ff26ffff04ff02ffff04ff05ff
    ff04ff37ffff04ff2fffff04ff5fffff04ff8201bfffff04ff82017fffff04ff
    8202ffff8080808080808080808080ff0180ffff01ff02ffff03ffff15ff8202
    ffffff11ff0bffff01018080ffff01ff04ffff04ff20ffff04ff82017fffff04
    ff5fff80808080ff8080ffff01ff088080ff018080ff0180ff0bff17ffff02ff
    5effff04ff02ffff04ff09ffff04ff2fffff04ffff02ff7effff04ff02ffff04
    ffff04ff09ffff04ff0bff1d8080ff80808080ff808080808080ff5f80ffff04
    ffff0101ffff04ffff04ff2cffff04ff05ff808080ffff04ffff04ff20ffff04
    ff17ffff04ff0bff80808080ff80808080ffff0bff2affff0bff22ff2480ffff
    0bff2affff0bff2affff0bff22ff3280ff0580ffff0bff2affff02ff3affff04
    ff02ffff04ff07ffff04ffff0bff22ff2280ff8080808080ffff0bff22ff8080
    808080ff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff7effff04ff
    02ffff04ff09ff80808080ffff02ff7effff04ff02ffff04ff0dff8080808080
    ffff01ff0bffff0101ff058080ff0180ff018080
    "
);

/// This is the puzzle hash of the [DID1 standard](https://chialisp.com/dids) puzzle.
pub const DID_INNER_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    33143d2bef64f14036742673afd158126b94284b4530a28c354fac202b0c910e
    "
));

#[cfg(test)]
mod tests {
    use clvm_traits::{clvm_list, match_list};
    use clvmr::{
        run_program,
        serde::{node_from_bytes, node_to_bytes},
        Allocator, ChiaDialect,
    };

    use super::*;

    use crate::assert_puzzle_hash;

    #[test]
    fn puzzle_hashes() {
        assert_puzzle_hash!(DID_INNER_PUZZLE => DID_INNER_PUZZLE_HASH);
    }

    #[test]
    fn did_solution() {
        let a = &mut Allocator::new();

        let ptr = clvm_list!(1, clvm_list!(42, "test")).to_clvm(a).unwrap();
        let did_solution = DidSolution::<match_list!(i32, String)>::from_clvm(a, ptr).unwrap();
        assert_eq!(
            did_solution,
            DidSolution::Spend(clvm_list!(42, "test".to_string()))
        );

        let puzzle = node_from_bytes(a, &DID_INNER_PUZZLE).unwrap();
        let curried = CurriedProgram {
            program: puzzle,
            args: DidArgs::new(1, None, 1, SingletonStruct::new(Bytes32::default()), ()),
        }
        .to_clvm(a)
        .unwrap();

        let output = run_program(a, &ChiaDialect::new(0), curried, ptr, u64::MAX)
            .expect("could not run did puzzle and solution");
        assert_eq!(
            hex::encode(node_to_bytes(a, output.1).unwrap()),
            "ff2aff847465737480"
        );
    }

    #[test]
    fn did_solution_roundtrip() {
        let a = &mut Allocator::new();
        let did_solution = DidSolution::Spend(a.nil());
        let ptr = did_solution.to_clvm(a).unwrap();
        let roundtrip = DidSolution::from_clvm(a, ptr).unwrap();
        assert_eq!(did_solution, roundtrip);
    }
}
