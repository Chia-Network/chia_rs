use std::sync::Arc;

use chia_protocol::CoinState;

pub fn select_coins(mut coin_state: Vec<Arc<CoinState>>, amount: u64) -> Vec<Arc<CoinState>> {
    coin_state.sort_by(|a, b| a.coin.amount.cmp(&b.coin.amount));

    let mut selected = Vec::new();
    let mut selected_amount = 0;

    for state in coin_state {
        if selected_amount >= amount {
            break;
        }
        selected_amount += state.coin.amount;
        selected.push(state);
    }

    if selected_amount < amount {
        Vec::new()
    } else {
        selected
    }
}
