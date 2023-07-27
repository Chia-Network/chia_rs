use crate::coin::Coin;
use crate::streamable_struct;
use chia_streamable_macro::Streamable;

streamable_struct! (CoinState {
    coin: Coin,
    spent_height: Option<u32>,
    created_height: Option<u32>,
});
