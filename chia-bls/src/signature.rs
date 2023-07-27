use blst::{
    blst_p2 as P2, blst_p2_add_affine, blst_p2_affine as P2Affine, blst_p2_affine_compress,
    blst_p2_affine_generator, blst_p2_affine_in_g2, blst_p2_affine_is_inf, blst_p2_from_affine,
    blst_p2_to_affine, blst_p2_uncompress, BLST_ERROR,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub(crate) P2Affine);

impl Signature {
    pub fn validate(bytes: &[u8; 96]) -> Option<Self> {
        let result = Self::from_bytes(bytes)?;

        if result.is_valid(true) {
            Some(result)
        } else {
            None
        }
    }

    pub fn from_bytes(bytes: &[u8; 96]) -> Option<Self> {
        if (bytes[0] & 0x80) != 0 {
            let mut p2_affine = P2Affine::default();
            if unsafe { blst_p2_uncompress(&mut p2_affine, bytes.as_ptr()) }
                == BLST_ERROR::BLST_SUCCESS
            {
                Some(Self(p2_affine))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn to_bytes(&self) -> [u8; 96] {
        let mut bytes = [0u8; 96];
        unsafe {
            blst_p2_affine_compress(bytes.as_mut_ptr(), &self.0);
        }
        bytes
    }

    pub fn is_valid(&self, check_infinity: bool) -> bool {
        unsafe {
            (!check_infinity || !blst_p2_affine_is_inf(&self.0)) && blst_p2_affine_in_g2(&self.0)
        }
    }

    pub fn add(&self, signature: &Self) -> Self {
        let mut p2 = P2::default();
        unsafe {
            blst_p2_from_affine(&mut p2, &self.0);
        }

        let mut output_p2 = P2::default();
        unsafe {
            blst_p2_add_affine(&mut output_p2, &p2, &signature.0);
        }

        let mut p2_affine = P2Affine::default();
        unsafe {
            blst_p2_to_affine(&mut p2_affine, &output_p2);
        }
        Self(p2_affine)
    }
}

impl Default for Signature {
    fn default() -> Self {
        Self(unsafe { *blst_p2_affine_generator() })
    }
}

#[cfg(test)]
mod tests {
    use crate::{SecretKey, Signature};

    use bip39::Mnemonic;

    #[test]
    fn quick_test() {
        let bytes: [u8; 32] = rand::random();
        let mnemonic = Mnemonic::from_entropy(&bytes).unwrap();
        let seed = mnemonic.to_seed("");
        let secret_key = SecretKey::from_seed(&seed);
        let public_key = secret_key.to_public_key();

        let message_1 = &[1, 2, 3];
        let message_2 = &[4, 5, 6];

        let signature_1 = secret_key.sign(message_1);
        let signature_2 = secret_key.sign(message_2);

        assert!(public_key.verify(message_1, &signature_1));
        assert!(public_key.verify(message_2, &signature_2));
        assert!(!public_key.verify(message_2, &signature_1));
        assert!(!public_key.verify(message_1, &signature_2));

        assert_eq!(
            Signature::from_bytes(&signature_1.to_bytes()).unwrap(),
            signature_1
        );
    }
}
