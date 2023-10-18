#![no_main]
use ::chia_protocol::*;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;
use sha2::{Digest, Sha256};

#[cfg(fuzzing)]
use arbitrary::{Arbitrary, Unstructured};

pub fn test_streamable<T: Streamable + std::fmt::Debug + PartialEq>(obj: &T) {
    let bytes = obj.to_bytes().unwrap();
    let obj2 = match T::from_bytes(&bytes) {
        Err(_) => {
            panic!(
                "failed to parse input: {}, from object: {:?}",
                hex::encode(bytes),
                &obj
            );
        }
        Ok(o) => o,
    };
    assert_eq!(obj, &obj2);

    let mut ctx = Sha256::new();
    ctx.update(bytes);
    let expect_hash: [u8; 32] = ctx.finalize().into();
    assert_eq!(obj.hash(), expect_hash);
}
#[cfg(fuzzing)]
fn test<'a, T: Arbitrary<'a> + Streamable + std::fmt::Debug + PartialEq>(data: &'a [u8]) {
    let mut u = Unstructured::new(data);
    let obj = <T as Arbitrary<'a>>::arbitrary(&mut u).unwrap();
    test_streamable(&obj);
}

// this is here to make clippy happy
#[cfg(not(fuzzing))]
fn test<T: Streamable + std::fmt::Debug + PartialEq>(_data: &[u8]) {}

fuzz_target!(|data: &[u8]| {
    test::<Program>(data);
    test::<Message>(data);
    test::<ClassgroupElement>(data);
    test::<Coin>(data);
    test::<CoinSpend>(data);
    test::<CoinState>(data);
    test::<EndOfSubSlotBundle>(data);
    test::<FeeRate>(data);
    test::<FeeEstimate>(data);
    test::<FeeEstimateGroup>(data);
    test::<TransactionsInfo>(data);
    test::<FoliageTransactionBlock>(data);
    test::<FoliageBlockData>(data);
    test::<Foliage>(data);
    test::<FullBlock>(data);
    test::<HeaderBlock>(data);
    test::<PoolTarget>(data);
    test::<ProofOfSpace>(data);
    test::<RewardChainBlockUnfinished>(data);
    test::<RewardChainBlock>(data);
    test::<ChallengeBlockInfo>(data);
    test::<ChallengeChainSubSlot>(data);
    test::<InfusedChallengeChainSubSlot>(data);
    test::<RewardChainSubSlot>(data);
    test::<SubSlotProofs>(data);
    test::<SpendBundle>(data);
    test::<VDFInfo>(data);
    test::<VDFProof>(data);
    test::<PuzzleSolutionResponse>(data);
    test::<SubSlotData>(data);
    test::<SubEpochChallengeSegment>(data);
    test::<SubEpochSegments>(data);

    test::<Handshake>(data);

    // Wallet Protocol
    test::<RequestPuzzleSolution>(data);
    test::<RespondPuzzleSolution>(data);
    test::<RejectPuzzleSolution>(data);
    test::<SendTransaction>(data);
    test::<TransactionAck>(data);
    test::<NewPeakWallet>(data);
    test::<RequestBlockHeader>(data);
    test::<RespondBlockHeader>(data);
    test::<RejectHeaderRequest>(data);
    test::<RequestRemovals>(data);
    test::<RespondRemovals>(data);
    test::<RejectRemovalsRequest>(data);
    test::<RequestAdditions>(data);
    test::<RespondAdditions>(data);
    test::<RejectAdditionsRequest>(data);
    test::<RespondBlockHeaders>(data);
    test::<RejectBlockHeaders>(data);
    test::<RequestBlockHeaders>(data);
    test::<RequestHeaderBlocks>(data);
    test::<RejectHeaderBlocks>(data);
    test::<RespondHeaderBlocks>(data);
    test::<RegisterForPhUpdates>(data);
    test::<RespondToPhUpdates>(data);
    test::<RegisterForCoinUpdates>(data);
    test::<RespondToCoinUpdates>(data);
    test::<CoinStateUpdate>(data);
    test::<RequestChildren>(data);
    test::<RespondChildren>(data);
    test::<RequestSesInfo>(data);
    test::<RespondSesInfo>(data);
    test::<RequestFeeEstimates>(data);
    test::<RespondFeeEstimates>(data);
});