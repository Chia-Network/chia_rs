use blst::{blst_pairing as BlstPairing, blst_pairing_chk_n_aggr_pk_in_g1, blst_pairing_init};

use crate::{aug_scheme::DST, PublicKey, Signature};

#[derive(Debug)]
pub struct Pairing(BlstPairing);

impl Pairing {
    pub fn new() -> Self {
        let mut pairing = BlstPairing::default();
        unsafe {
            blst_pairing_init(&mut pairing, true, DST.as_ptr(), DST.len());
        }
        Self(pairing)
    }

    pub fn aggregate(
        &mut self,
        public_key: Option<&PublicKey>,
        validate_pk: bool,
        signature: Option<&Signature>,
        validate_sig: bool,
        message: &[u8],
    ) {
        unsafe {
            blst_pairing_chk_n_aggr_pk_in_g1(
                &mut self.0,
                match public_key {
                    Some(public_key) => &public_key.0,
                    None => std::ptr::null(),
                },
                validate_pk,
                match signature {
                    Some(signature) => &signature.0,
                    None => std::ptr::null(),
                },
                validate_sig,
                message.as_ptr(),
                message.len(),
                [].as_ptr(),
                0,
            );
        }
    }
}
