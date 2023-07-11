use std::sync::Arc;

use chia_protocol::CoinState;

#[derive(Default)]
pub struct CoinStore {
    coin_records: Vec<Arc<CoinState>>,
}

impl CoinStore {
    pub fn update(&mut self, new_state: Vec<CoinState>) {
        for updated_item in new_state {
            match self
                .coin_records
                .iter_mut()
                .find(|item| item.coin == updated_item.coin)
            {
                Some(existing) => *existing = Arc::new(updated_item),
                None => self.coin_records.push(Arc::new(updated_item)),
            }
        }
    }

    pub fn unspent(&self) -> Vec<Arc<CoinState>> {
        self.coin_records
            .iter()
            .filter_map(|record| {
                if record.spent_height.is_none() {
                    Some(Arc::clone(record))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn is_used(&self, puzzle_hash: &[u8; 32]) -> bool {
        self.coin_records
            .iter()
            .find(|record| record.coin.puzzle_hash == puzzle_hash)
            .is_some()
    }
}
