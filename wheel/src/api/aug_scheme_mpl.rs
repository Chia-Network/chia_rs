use std::iter::zip;

use chia_bls::{hash_to_g2, DerivableKey, PublicKey, SecretKey, Signature};
use chia_traits::{Bytes, Int, StubBuilder, TypeStub};
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyList};

#[pyclass]
pub struct AugSchemeMPL {}

#[pymethods]
impl AugSchemeMPL {
    #[staticmethod]
    #[pyo3(signature = (pk,msg,prepend_pk=None))]
    pub fn sign(pk: &SecretKey, msg: &[u8], prepend_pk: Option<&PublicKey>) -> Signature {
        match prepend_pk {
            Some(prefix) => {
                let mut aug_msg = prefix.to_bytes().to_vec();
                aug_msg.extend_from_slice(msg);
                chia_bls::sign_raw(pk, aug_msg)
            }
            None => chia_bls::sign(pk, msg),
        }
    }

    #[staticmethod]
    pub fn aggregate(sigs: &Bound<'_, PyList>) -> PyResult<Signature> {
        let mut ret = Signature::default();
        for p2 in sigs {
            ret += &p2.extract::<Signature>()?;
        }
        Ok(ret)
    }

    #[staticmethod]
    pub fn verify(pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
        chia_bls::verify(sig, pk, msg)
    }

    #[staticmethod]
    pub fn aggregate_verify(
        pks: &Bound<'_, PyList>,
        msgs: &Bound<'_, PyList>,
        sig: &Signature,
    ) -> PyResult<bool> {
        let mut data = Vec::<(PublicKey, Vec<u8>)>::new();
        if pks.len() != msgs.len() {
            return Err(PyRuntimeError::new_err(
                "aggregate_verify expects the same number of public keys as messages",
            ));
        }
        for (pk, msg) in zip(pks, msgs) {
            let pk = pk.extract::<PublicKey>()?;
            let msg = msg.extract::<Vec<u8>>()?;
            data.push((pk, msg));
        }

        Ok(chia_bls::aggregate_verify(sig, data))
    }

    #[staticmethod]
    pub fn g2_from_message(msg: &[u8]) -> Signature {
        hash_to_g2(msg)
    }

    #[staticmethod]
    pub fn derive_child_sk(sk: &SecretKey, index: u32) -> SecretKey {
        sk.derive_hardened(index)
    }

    #[staticmethod]
    pub fn derive_child_sk_unhardened(sk: &SecretKey, index: u32) -> SecretKey {
        sk.derive_unhardened(index)
    }

    #[staticmethod]
    pub fn derive_child_pk_unhardened(pk: &PublicKey, index: u32) -> PublicKey {
        pk.derive_unhardened(index)
    }

    #[staticmethod]
    pub fn key_gen(seed: &[u8]) -> PyResult<SecretKey> {
        if seed.len() < 32 {
            return Err(PyRuntimeError::new_err(
                "Seed size must be at leat 32 bytes",
            ));
        }
        Ok(SecretKey::from_seed(seed))
    }
}

impl TypeStub for AugSchemeMPL {
    fn type_stub(builder: &StubBuilder) -> String {
        if !builder.has("AugSchemeMPL") {
            builder
                .class::<Self>("AugSchemeMPL")
                .static_method::<Signature>("sign", |m| {
                    m.param::<SecretKey>("sk")
                        .param::<Bytes>("msg")
                        .default_param::<Option<PublicKey>>("prepend_pk", "None")
                })
                .static_method::<Signature>("aggregate", |m| m.param::<Vec<Signature>>("sigs"))
                .static_method::<bool>("verify", |m| {
                    m.param::<PublicKey>("pk")
                        .param::<Bytes>("msg")
                        .param::<Signature>("sig")
                })
                .static_method::<bool>("aggregate_verify", |m| {
                    m.param::<Vec<PublicKey>>("pks")
                        .param::<Vec<Bytes>>("msgs")
                        .param::<Signature>("sig")
                })
                .static_method::<SecretKey>("key_gen", |m| m.param::<Bytes>("seed"))
                .static_method::<Signature>("g2_from_message", |m| m.param::<Bytes>("msg"))
                .static_method::<SecretKey>("derive_child_sk", |m| {
                    m.param::<SecretKey>("sk").param::<Int>("index")
                })
                .static_method::<SecretKey>("derive_child_sk_unhardened", |m| {
                    m.param::<SecretKey>("sk").param::<Int>("index")
                })
                .static_method::<PublicKey>("derive_child_pk_unhardened", |m| {
                    m.param::<PublicKey>("pk").param::<Int>("index")
                })
                .generate();
        }
        "AugSchemeMPL".to_string()
    }
}
