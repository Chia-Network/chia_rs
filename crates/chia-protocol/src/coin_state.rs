use crate::coin::Coin;
use chia_streamable_macro::streamable;

#[streamable]
#[derive(Copy)]
#[cfg_attr(
    feature = "serde",
    serde_with::serde_as,
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct CoinState {
    coin: Coin,
    spent_height: Option<u32>,
    created_height: Option<u32>,
}
