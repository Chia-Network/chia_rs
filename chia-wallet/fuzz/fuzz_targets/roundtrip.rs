#![no_main]
use std::fmt;

use chia_wallet::{nft::NftMetadata, Proof};
use clvm_traits::{AllocatorExt, FromClvm, ToClvm};
use clvmr::{allocator::NodePtr, Allocator};
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
    T: Arbitrary<'a> + ToClvm<NodePtr> + FromClvm<NodePtr> + PartialEq + fmt::Debug,
{
    let obj = T::arbitrary(u).unwrap();
    let mut a = Allocator::new();
    let ptr = a.value_to_ptr(&obj).unwrap();
    let obj2 = a.value_from_ptr::<T>(ptr).unwrap();
    assert_eq!(obj, obj2);
}
