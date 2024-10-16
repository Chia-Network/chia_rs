use crate::coin::Coin;
use chia_streamable_macro::streamable;

#[streamable]
#[derive(Copy)]
pub struct CoinState {
    coin: Coin,
    spent_height: Option<u32>,
    created_height: Option<u32>,
}
