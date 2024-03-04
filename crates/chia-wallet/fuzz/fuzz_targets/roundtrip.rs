#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    #[cfg(fuzzing)]
    {
        use std::fmt;

        use chia_wallet::{nft::NftMetadata, Proof};
        use clvm_traits::{FromClvm, ToClvm};
        use clvmr::{allocator::NodePtr, Allocator};
        use libfuzzer_sys::arbitrary::{Arbitrary, Unstructured};

        let mut u = Unstructured::new(data);
        roundtrip::<NftMetadata>(&mut u);
        roundtrip::<Proof>(&mut u);

        fn roundtrip<'a, T>(u: &mut Unstructured<'a>)
        where
            T: Arbitrary<'a> + ToClvm<NodePtr> + FromClvm<NodePtr> + PartialEq + fmt::Debug,
        {
            let obj = T::arbitrary(u).unwrap();
            let mut a = Allocator::new();
            let ptr = obj.to_clvm(&mut a).unwrap();
            let obj2 = T::from_clvm(&a, ptr).unwrap();
            assert_eq!(obj, obj2);
        }
    }

    #[cfg(not(fuzzing))]
    {
        let _ = data;
    }
});
