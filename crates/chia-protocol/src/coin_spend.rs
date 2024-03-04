use chia_streamable_macro::Streamable;

use crate::coin::Coin;
use crate::program::Program;
use crate::streamable_struct;

streamable_struct!(CoinSpend {
    coin: Coin,
    puzzle_reveal: Program,
    solution: Program,
});
