#![no_main]
use libfuzzer_sys::fuzz_target;

#[cfg(fuzzing)]
use arbitrary::{Arbitrary, Unstructured};

fuzz_target!(|data: &[u8]| {
    test(data);
});

#[cfg(fuzzing)]
fn test(data: &[u8]) {
    use chia_wallet::nft::NftMetadata;
    use clvm_traits::{FromClvm, ToClvm};
    use clvmr::Allocator;

    let mut u = Unstructured::new(data);
    let obj = <NftMetadata as Arbitrary>::arbitrary(&mut u).unwrap();

    let mut a = Allocator::new();
    let ptr = obj.to_clvm(&mut a).unwrap();
    let obj2 = NftMetadata::from_clvm(&a, ptr).unwrap();

    assert_eq!(obj, obj2);
}

#[cfg(not(fuzzing))]
fn test(_data: &[u8]) {}
