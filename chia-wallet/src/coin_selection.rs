use chia_protocol::Coin;

pub fn select_coins(mut coins: Vec<Coin>, amount: u64) -> Vec<Coin> {
    coins.sort_by(|a, b| a.amount.cmp(&b.amount));

    let mut selected = Vec::new();
    let mut selected_amount = 0;

    for coin in coins {
        if selected_amount >= amount {
            break;
        }
        selected_amount += coin.amount;
        selected.push(coin);
    }

    if selected_amount < amount {
        Vec::new()
    } else {
        selected
    }
}
