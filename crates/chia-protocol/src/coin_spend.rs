use chia_streamable_macro::streamable;

use crate::coin::Coin;
use crate::program::Program;

#[streamable]
#[cfg_attr(
    feature = "serde",
    serde_with::serde_as,
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct CoinSpend {
    coin: Coin,
    puzzle_reveal: Program,
    solution: Program,
}
