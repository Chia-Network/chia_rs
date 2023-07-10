use chia_protocol::CoinState;

#[derive(Default)]
pub struct CoinStore {
    pub coin_state: Vec<CoinState>,
}

impl CoinStore {
    pub fn update(&mut self, new_state: Vec<CoinState>) {
        for updated_item in new_state {
            match self
                .coin_state
                .iter_mut()
                .find(|item| item.coin == updated_item.coin)
            {
                Some(existing) => *existing = updated_item,
                None => self.coin_state.push(updated_item),
            }
        }
    }
}
