use chia_streamable_macro::Streamable;

use crate::streamable_struct;
use crate::Bytes32;

streamable_struct!(PoolTarget {
    puzzle_hash: Bytes32,
    max_height: u32, // A max height of 0 means it is valid forever
});
