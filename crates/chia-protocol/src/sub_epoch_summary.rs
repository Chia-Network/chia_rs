use chia_sha2::Sha256;
use chia_streamable_macro::streamable;
use chia_traits::{Result, Streamable};
use std::io::Cursor;

use crate::Bytes32;
use crate::utils::{parse, stream, update_digest};

#[streamable(no_streamable)]
pub struct SubEpochSummary {
    prev_subepoch_summary_hash: Bytes32,
    reward_chain_hash: Bytes32, // hash of reward chain at end of last segment
    num_blocks_overflow: u8,    // How many more blocks than 384*(N-1)
    new_difficulty: Option<u64>, // Only once per epoch (diff adjustment)
    new_sub_slot_iters: Option<u64>, // Only once per epoch (diff adjustment)
    // MMR root of all challenge chain hashes in this sub-epoch (None for pre-fork)
    challenge_merkle_root: Option<Bytes32>,
}

impl Streamable for SubEpochSummary {
    fn update_digest(&self, digest: &mut Sha256) {
        self.prev_subepoch_summary_hash.update_digest(digest);
        self.reward_chain_hash.update_digest(digest);
        self.num_blocks_overflow.update_digest(digest);
        self.new_difficulty.update_digest(digest);
        update_digest(
            self.new_sub_slot_iters.as_ref(),
            self.challenge_merkle_root.as_ref(),
            digest,
        );
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.prev_subepoch_summary_hash.stream(out)?;
        self.reward_chain_hash.stream(out)?;
        self.num_blocks_overflow.stream(out)?;
        self.new_difficulty.stream(out)?;
        stream(
            self.new_sub_slot_iters.as_ref(),
            self.challenge_merkle_root.as_ref(),
            out,
        )
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let psesh = <Bytes32 as Streamable>::parse::<TRUSTED>(input)?;
        let rch = <Bytes32 as Streamable>::parse::<TRUSTED>(input)?;
        let nbo = <u8 as Streamable>::parse::<TRUSTED>(input)?;
        let nd = <Option<u64> as Streamable>::parse::<TRUSTED>(input)?;
        let (nssi, challenge_merkle_root) = parse::<TRUSTED, u64, Bytes32>(input)?;
        Ok(Self {
            prev_subepoch_summary_hash: psesh,
            reward_chain_hash: rch,
            num_blocks_overflow: nbo,
            new_difficulty: nd,
            new_sub_slot_iters: nssi,
            challenge_merkle_root,
        })
    }
}
