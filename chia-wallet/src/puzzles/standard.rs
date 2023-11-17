use chia_bls::PublicKey;
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::{curry_tree_hash, tree_hash_atom};
use hex_literal::hex;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curry)]
pub struct StandardArgs {
    pub synthetic_key: PublicKey,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
pub struct StandardSolution<P, S> {
    pub original_public_key: Option<PublicKey>,
    pub delegated_puzzle: P,
    pub solution: S,
}

pub fn standard_puzzle_hash(synthetic_key: &PublicKey) -> [u8; 32] {
    let sk_tree_hash = tree_hash_atom(&synthetic_key.to_bytes());
    curry_tree_hash(STANDARD_PUZZLE_HASH, &[sk_tree_hash])
}

/// This is the puzzle reveal of the [standard transaction](https://chialisp.com/standard-transactions) puzzle.
pub static STANDARD_PUZZLE: [u8; 227] = hex!(
    "
    ff02ffff01ff02ffff03ff0bffff01ff02ffff03ffff09ff05ffff1dff0bffff
    1effff0bff0bffff02ff06ffff04ff02ffff04ff17ff8080808080808080ffff
    01ff02ff17ff2f80ffff01ff088080ff0180ffff01ff04ffff04ff04ffff04ff
    05ffff04ffff02ff06ffff04ff02ffff04ff17ff80808080ff80808080ffff02
    ff17ff2f808080ff0180ffff04ffff01ff32ff02ffff03ffff07ff0580ffff01
    ff0bffff0102ffff02ff06ffff04ff02ffff04ff09ff80808080ffff02ff06ff
    ff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff
    018080
    "
);

/// This is the puzzle hash of the [standard transaction](https://chialisp.com/standard-transactions) puzzle.
pub static STANDARD_PUZZLE_HASH: [u8; 32] = hex!(
    "
    e9aaa49f45bad5c889b86ee3341550c155cfdd10c3a6757de618d20612fffd52
    "
);

/// This is the puzzle reveal of the [default hidden puzzle](https://chialisp.com/standard-transactions#default-hidden-puzzle).
pub static DEFAULT_HIDDEN_PUZZLE: [u8; 3] = hex!("ff0980");

/// This is the puzzle hash of the [default hidden puzzle](https://chialisp.com/standard-transactions#default-hidden-puzzle).
pub static DEFAULT_HIDDEN_PUZZLE_HASH: [u8; 32] = hex!(
    "
    711d6c4e32c92e53179b199484cf8c897542bc57f2b22582799f9d657eec4699
    "
);

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_puzzle_hash;

    #[test]
    fn puzzle_hashes() {
        assert_puzzle_hash!(STANDARD_PUZZLE => STANDARD_PUZZLE_HASH);
        assert_puzzle_hash!(DEFAULT_HIDDEN_PUZZLE => DEFAULT_HIDDEN_PUZZLE_HASH);
    }
}
