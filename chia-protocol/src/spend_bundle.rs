use crate::coin_spend::CoinSpend;
use crate::streamable_struct;
use chia_bls::G2Element;
use chia_streamable_macro::Streamable;

streamable_struct! (SpendBundle {
    coin_spends: Vec<CoinSpend>,
    aggregated_signature: G2Element,
});
