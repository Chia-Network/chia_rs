use crate::streamable_struct;
use chia_streamable_macro::Streamable;

use crate::bytes::Bytes96;
use crate::coin_spend::CoinSpend;

streamable_struct! (SpendBundle {
    coin_spends: Vec<CoinSpend>,
    aggregated_signature: Bytes96,
});
