#![no_main]

use clvm_traits::{decode_number, encode_number};
use clvmr::Allocator;
use libfuzzer_sys::{arbitrary::Unstructured, fuzz_target};

fuzz_target!(|data: &[u8]| {
    let mut unstructured = Unstructured::new(data);

    macro_rules! impl_num {
        ( $num_type:ty, $signed:expr ) => {
            let num: $num_type = unstructured.arbitrary().unwrap();
            let mut allocator = Allocator::new();
            let ptr = allocator.new_number(num.into()).unwrap();
            let atom = allocator.atom(ptr);
            let expected = atom.as_ref();

            #[allow(unused_comparisons)]
            let encoded = encode_number(&num.to_be_bytes(), num < 0);
            assert_eq!(expected, encoded);

            let expected = num.to_be_bytes();
            let decoded = decode_number(&encoded, $signed).unwrap();
            assert_eq!(expected, decoded);
        };
    }

    impl_num!(u8, false);
    impl_num!(i8, true);
    impl_num!(u16, false);
    impl_num!(i16, true);
    impl_num!(u32, false);
    impl_num!(i32, true);
    impl_num!(u64, false);
    impl_num!(i64, true);
    impl_num!(u128, false);
    impl_num!(i128, true);
    impl_num!(usize, false);
    impl_num!(isize, true);
});
