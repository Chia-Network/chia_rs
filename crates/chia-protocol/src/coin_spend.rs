use chia_streamable_macro::{streamable, Streamable};

use crate::coin::Coin;
use crate::program::Program;

#[streamable]
pub struct CoinSpend {
    coin: Coin,
    puzzle_reveal: Program,
    solution: Program,
}
