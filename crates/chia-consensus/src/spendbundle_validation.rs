use clvmr::{ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV};

use crate::consensus_constants::ConsensusConstants;
use crate::gen::flags::{ALLOW_BACKREFS, DISALLOW_INFINITY_G1, ENABLE_MESSAGE_CONDITIONS};

pub fn get_flags_for_height_and_constants(height: u32, constants: &ConsensusConstants) -> u32 {
    let mut flags: u32 = 0;

    if height >= constants.soft_fork4_height {
        flags |= ENABLE_MESSAGE_CONDITIONS;
    }

    if height >= constants.soft_fork5_height {
        flags |= DISALLOW_INFINITY_G1;
    }

    if height >= constants.hard_fork_height {
        //  the hard-fork initiated with 2.0. To activate June 2024
        //  * costs are ascribed to some unknown condition codes, to allow for
        // soft-forking in new conditions with cost
        //  * a new condition, SOFTFORK, is added which takes a first parameter to
        //    specify its cost. This allows soft-forks similar to the softfork
        //    operator
        //  * BLS operators introduced in the soft-fork (behind the softfork
        //    guard) are made available outside of the guard.
        //  * division with negative numbers are allowed, and round toward
        //    negative infinity
        //  * AGG_SIG_* conditions are allowed to have unknown additional
        //    arguments
        //  * Allow the block generator to be serialized with the improved clvm
        //   serialization format (with back-references)
        flags = flags | ENABLE_BLS_OPS_OUTSIDE_GUARD | ENABLE_FIXED_DIV | ALLOW_BACKREFS;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use rstest::rstest;

    #[rstest]
    #[case(0, 0)]
    #[case(TEST_CONSTANTS.hard_fork_height, ENABLE_BLS_OPS_OUTSIDE_GUARD | ENABLE_FIXED_DIV | ALLOW_BACKREFS)]
    #[case(TEST_CONSTANTS.soft_fork4_height, ENABLE_BLS_OPS_OUTSIDE_GUARD | ENABLE_FIXED_DIV | ALLOW_BACKREFS | ENABLE_MESSAGE_CONDITIONS)]
    #[case(TEST_CONSTANTS.soft_fork5_height, ENABLE_BLS_OPS_OUTSIDE_GUARD | ENABLE_FIXED_DIV | ALLOW_BACKREFS | ENABLE_MESSAGE_CONDITIONS | DISALLOW_INFINITY_G1)]
    fn test_get_flags(#[case] height: u32, #[case] expected_value: u32) {
        assert_eq!(
            get_flags_for_height_and_constants(height, &TEST_CONSTANTS),
            expected_value
        );
    }
}
