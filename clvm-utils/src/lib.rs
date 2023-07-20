pub use clvm_macros::*;

mod curry;
mod curry_tree_hash;
mod new_list;
mod traits;
mod tree_hash;
mod uncurry;

pub use curry::*;
pub use curry_tree_hash::*;
pub use new_list::*;
pub use traits::*;
pub use tree_hash::*;
pub use uncurry::*;

#[cfg(test)]
mod tests {
    use clvmr::Allocator;

    use super::*;

    #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
    struct TupleStruct {
        a: u64,
        b: i32,
    }

    #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
    #[clvm(proper_list)]
    struct ProperListStruct {
        a: u64,
        b: i32,
    }

    #[test]
    fn test_round_trip_tuple() {
        let mut a = Allocator::new();
        let tuple_struct = TupleStruct { a: 52, b: -32 };
        let node = tuple_struct.to_clvm(&mut a).unwrap();
        let round_trip = TupleStruct::from_clvm(&a, node).unwrap();
        assert_eq!(tuple_struct, round_trip);
    }

    #[test]
    fn test_round_trip_proper_list() {
        let mut a = Allocator::new();
        let tuple_struct = ProperListStruct { a: 52, b: -32 };
        let node = tuple_struct.to_clvm(&mut a).unwrap();
        let round_trip = ProperListStruct::from_clvm(&a, node).unwrap();
        assert_eq!(tuple_struct, round_trip);
    }
}
