#![no_main]
use arbitrary::{Arbitrary, Unstructured};
use chia_protocol::*;
use chia_sha2::Sha256;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;

pub fn test_streamable<T: Streamable + std::fmt::Debug + PartialEq>(obj: &T) {
    let bytes = obj.to_bytes().unwrap();
    let Ok(obj2) = T::from_bytes(&bytes) else {
        panic!(
            "failed to parse input: {}, from object: {:?}",
            hex::encode(&bytes),
            &obj
        )
    };
    assert_eq!(obj, &obj2);

    let obj3 = T::from_bytes_unchecked(&bytes).unwrap();
    assert_eq!(obj, &obj3);

    let mut ctx = Sha256::new();
    ctx.update(&bytes);
    let expect_hash: [u8; 32] = ctx.finalize();
    assert_eq!(obj.hash(), expect_hash);

    // make sure input too large is an error
    let mut corrupt_bytes = bytes.clone();
    corrupt_bytes.push(0);
    assert!(T::from_bytes_unchecked(&corrupt_bytes) == Err(chia_traits::Error::InputTooLarge));

    if !bytes.is_empty() {
        // make sure input too short is an error
        corrupt_bytes.truncate(bytes.len() - 1);
        assert!(T::from_bytes_unchecked(&corrupt_bytes) == Err(chia_traits::Error::EndOfBuffer));
    }
}
fn test<'a, T: Arbitrary<'a> + Streamable + std::fmt::Debug + PartialEq>(data: &'a [u8]) {
    let mut u = Unstructured::new(data);
    let obj = <T as Arbitrary<'a>>::arbitrary(&mut u).unwrap();
    test_streamable(&obj);

    // ensure parsing from garbage bytes doesn't crash or panic
    let _ = Foliage::from_bytes(data);
}

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
    test::<BlockRecord>(data);
    test::<UnfinishedBlock>(data);
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
    test::<SubEpochSummary>(data);
    test::<WeightProof>(data);
    test::<TimestampedPeerInfo>(data);
    test::<RecentChainData>(data);
    test::<ProofBlockHeader>(data);
    test::<SubEpochData>(data);

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

    // Full Node Protocol
    test::<NewPeak>(data);
    test::<NewTransaction>(data);
    test::<RequestTransaction>(data);
    test::<RespondTransaction>(data);
    test::<RequestProofOfWeight>(data);
    test::<RespondProofOfWeight>(data);
    test::<RequestBlock>(data);
    test::<RejectBlock>(data);
    test::<RequestBlocks>(data);
    test::<RespondBlocks>(data);
    test::<RejectBlocks>(data);
    test::<RespondBlock>(data);
    test::<NewUnfinishedBlock>(data);
    test::<RequestUnfinishedBlock>(data);
    test::<RespondUnfinishedBlock>(data);
    test::<NewSignagePointOrEndOfSubSlot>(data);
    test::<RequestSignagePointOrEndOfSubSlot>(data);
    test::<RespondSignagePoint>(data);
    test::<RespondEndOfSubSlot>(data);
    test::<RequestMempoolTransactions>(data);
    test::<NewCompactVDF>(data);
    test::<RequestCompactVDF>(data);
    test::<RespondCompactVDF>(data);
    test::<RequestPeers>(data);
    test::<RespondPeers>(data);
});
