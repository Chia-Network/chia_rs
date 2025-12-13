#![no_main]

use std::fmt;

use chia_puzzle_types::{Proof, nft::NftMetadata};
use clvm_traits::{FromClvm, ToClvm};
use clvmr::Allocator;
use libfuzzer_sys::arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    roundtrip::<NftMetadata>(&mut u);
    roundtrip::<Proof>(&mut u);
});

fn roundtrip<'a, T>(u: &mut Unstructured<'a>)
where
    T: Arbitrary<'a> + ToClvm<Allocator> + FromClvm<Allocator> + PartialEq + fmt::Debug,
{
    let obj = T::arbitrary(u).unwrap();
    let mut a = Allocator::new();
    let ptr = obj.to_clvm(&mut a).unwrap();
    let obj2 = T::from_clvm(&a, ptr).unwrap();
    assert_eq!(obj, obj2);
}
