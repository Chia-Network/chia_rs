#![no_main]

use chia_wallet::{nft::NftMetadata, Proof};
use libfuzzer_sys::{arbitrary::Unstructured, fuzz_target};

#[cfg(fuzzing)]
use clvm_traits::{FromPtr, ToPtr};
#[cfg(fuzzing)]
use clvmr::Allocator;
#[cfg(fuzzing)]
use libfuzzer_sys::arbitrary::Arbitrary;
#[cfg(fuzzing)]
use std::fmt;

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    roundtrip::<NftMetadata>(&mut u);
    roundtrip::<Proof>(&mut u);
});

#[cfg(fuzzing)]
fn roundtrip<'a, T>(u: &mut Unstructured<'a>)
where
    T: Arbitrary<'a> + ToPtr + FromPtr + PartialEq + fmt::Debug,
{
    let obj = T::arbitrary(u).unwrap();
    let mut a = Allocator::new();
    let ptr = obj.to_ptr(&mut a).unwrap();
    let obj2 = T::from_ptr(&a, ptr).unwrap();
    assert_eq!(obj, obj2);
}

#[cfg(not(fuzzing))]
fn roundtrip<T>(_u: &mut Unstructured) {}
