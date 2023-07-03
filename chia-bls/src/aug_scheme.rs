use crate::PublicKey;

use blst::{blst_hash_to_g2, blst_p2 as P2};

pub const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_AUG_";

pub fn prepend_message(public_key: &PublicKey, message: &[u8]) -> Vec<u8> {
    let mut prepended = public_key.to_bytes().to_vec();
    prepended.extend_from_slice(message.as_ref());
    prepended
}

pub fn hash_to_g2(message: &[u8]) -> P2 {
    let mut p2 = P2::default();
    unsafe {
        blst_hash_to_g2(
            &mut p2,
            message.as_ptr(),
            message.len(),
            DST.as_ptr(),
            DST.len(),
            [].as_ptr(),
            0,
        );
    }
    p2
}
