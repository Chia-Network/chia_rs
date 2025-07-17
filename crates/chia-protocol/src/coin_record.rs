use chia_streamable_macro::streamable;
use pyo3::pymethods;

use crate::{Bytes32, Coin, CoinState};

#[streamable]
#[derive(Copy)]
pub struct CoinRecord {
    // These are values that correspond to a CoinName that are used
    // in keeping track of the unspent database.
    coin: Coin,
    confirmed_block_index: u32,
    spent_block_index: u32,
    coinbase: bool,
    timestamp: u64, // Timestamp of the block at height confirmed_block_index
}

#[pymethods]
impl CoinRecord {
    pub fn spent(&self) -> bool {
        self.spent_block_index > 0
    }

    pub fn name(&self) -> Bytes32 {
        self.coin.coin_id()
    }

    pub fn coin_state(&self) -> CoinState {
        let spent_h = if self.spent() {
            Some(self.spent_block_index)
        } else {
            None
        };

        let confirmed_height = if self.confirmed_block_index == 0_u32 && self.timestamp == 0_u64 {
            None
        } else {
            Some(self.confirmed_block_index)
        };

        CoinState::new(self.coin, spent_h, confirmed_height)
    }
}
