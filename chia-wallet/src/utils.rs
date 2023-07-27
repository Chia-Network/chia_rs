use chia_protocol::{Coin, CoinState};
use num_bigint::BigInt;

pub fn int_to_bytes(item: BigInt) -> Vec<u8> {
    let bytes: Vec<u8> = item.to_signed_bytes_be();
    let mut slice = bytes.as_slice();
    while !slice.is_empty() && slice[0] == 0 {
        if slice.len() > 1 && (slice[1] & 0x80 == 0x80) {
            break;
        }
        slice = &slice[1..];
    }
    slice.to_vec()
}

pub fn select_coins(mut coins: Vec<&Coin>, amount: u64) -> Vec<&Coin> {
    coins.sort_by(|a, b| a.amount.cmp(&b.amount));

    let mut result = Vec::new();
    let mut selected = 0u128;

    for coin in coins {
        if selected >= amount as u128 {
            break;
        }

        selected += coin.amount as u128;
        result.push(coin);
    }

    if selected >= amount as u128 {
        result
    } else {
        Vec::new()
    }
}

pub fn update_state(state: &mut Vec<CoinState>, update: CoinState) {
    match state.iter_mut().find(|item| item.coin == update.coin) {
        Some(value) => *value = update,
        None => state.push(update),
    }
}
