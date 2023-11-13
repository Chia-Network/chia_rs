#![no_main]
use std::fmt;

use chia_wallet::{nft::NftMetadata, Proof};
use clvm_traits::{FromPtr, ToPtr};
use clvmr::Allocator;
use libfuzzer_sys::{
    arbitrary::{Arbitrary, Unstructured},
    fuzz_target,
};

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    roundtrip::<NftMetadata>(&mut u);
    roundtrip::<Proof>(&mut u);
});

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
