#[cfg(feature = "derive")]
pub use clvm_derive::*;

mod error;
mod from_clvm;
mod macros;
mod match_byte;
mod to_clvm;

pub use error::*;
pub use from_clvm::*;
pub use macros::*;
pub use match_byte::*;
pub use to_clvm::*;

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use clvmr::Allocator;

    use super::*;

    #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
    #[clvm(tuple)]
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

    #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
    #[clvm(curried_args)]
    struct CurriedArgsStruct {
        a: u64,
        b: i32,
    }

    #[test]
    fn test_round_trip_tuple() {
        let mut a = Allocator::new();
        let value = TupleStruct { a: 52, b: -32 };
        let node = value.to_clvm(&mut a).unwrap();
        let round_trip = TupleStruct::from_clvm(&a, node).unwrap();
        assert_eq!(value, round_trip);
    }

    #[test]
    fn test_round_trip_proper_list() {
        let mut a = Allocator::new();
        let value = ProperListStruct { a: 52, b: -32 };
        let node = value.to_clvm(&mut a).unwrap();
        let round_trip = ProperListStruct::from_clvm(&a, node).unwrap();
        assert_eq!(value, round_trip);
    }

    #[test]
    fn test_round_trip_curried_args() {
        let mut a = Allocator::new();
        let value = CurriedArgsStruct { a: 52, b: -32 };
        let node = value.to_clvm(&mut a).unwrap();
        let round_trip = CurriedArgsStruct::from_clvm(&a, node).unwrap();
        assert_eq!(value, round_trip);
    }
}
