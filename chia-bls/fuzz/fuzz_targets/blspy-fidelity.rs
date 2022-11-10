// PyO3 might use an unexpected version of python. So make sure blspy is
// installed in the one that's actually being used.
// On M1 MacOS, for example, pyO3 doesn't appear to be using the active venv,
// but the system Python3.9 executable (even though the default system python is
// 3.8.9). To install blspy:

// python3.9 -m pip install blspy

#![no_main]
use libfuzzer_sys::fuzz_target;
use pyo3::prelude::*;
use std::convert::TryFrom;

use chia_bls::secret_key::SecretKey;
use chia_bls::signature::{sign, aggregate};
use chia_bls::derivable_key::DerivableKey;
use pyo3::types::{PyTuple, PyBytes, PyList};

fn to_bytes<'a>(obj: &'a PyAny) -> &'a [u8] {
    obj.call_method0("__bytes__").unwrap().downcast::<PyBytes>().unwrap().as_bytes()
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 32 {
        return;
    }

    Python::with_gil(|py| {

        let blspy = py.import("blspy").unwrap();
        let aug = blspy.getattr("AugSchemeMPL").unwrap();

        // Generate key pair from seed
        let rust_sk = SecretKey::from_seed(data);
        let py_sk = aug.call_method1("key_gen", PyTuple::new(py, &[PyBytes::new(py, data)])).unwrap();

        // convert to bytes
        let rust_sk_bytes = rust_sk.to_bytes();
        let py_sk_bytes = to_bytes(py_sk);
        assert_eq!(py_sk_bytes, rust_sk_bytes);

        // get the public key
        let rust_pk = rust_sk.public_key();
        let py_pk = py_sk.call_method0("get_g1").unwrap();

        // convert to bytes
        let rust_pk_bytes = rust_pk.to_bytes();
        let py_pk_bytes = to_bytes(py_pk);
        assert_eq!(py_pk_bytes, rust_pk_bytes);

        let idx = u32::from_be_bytes(<[u8; 4]>::try_from(&data[0..4]).unwrap());
        let rust_sk1 = rust_sk.derive_unhardened(idx);
        let py_sk1 = aug.call_method1("derive_child_sk_unhardened",
            PyTuple::new(py, &[py_sk, idx.to_object(py).as_ref(py)])).unwrap();
        assert_eq!(to_bytes(py_sk1), rust_sk1.to_bytes());

        let rust_pk1 = rust_pk.derive_unhardened(idx);
        let py_pk1 = aug.call_method1("derive_child_pk_unhardened",
            PyTuple::new(py, &[py_pk, idx.to_object(py).as_ref(py)])).unwrap();
        assert_eq!(to_bytes(py_pk1), rust_pk1.to_bytes());

        // sign with the derived keys
        let rust_sig1 = sign(&rust_sk1, data);
        let py_sig1 = aug.call_method1("sign",
            PyTuple::new(py, &[py_sk1, PyBytes::new(py, data)])).unwrap();
        assert_eq!(to_bytes(py_sig1), rust_sig1.to_bytes());

        // derive hardened
        let idx = u32::from_be_bytes(<[u8; 4]>::try_from(&data[4..8]).unwrap());
        let rust_sk2 = rust_sk.derive_hardened(idx);
        let py_sk2 = aug.call_method1("derive_child_sk",
            PyTuple::new(py, &[py_sk, idx.to_object(py).as_ref(py)])).unwrap();
        assert_eq!(to_bytes(py_sk2), rust_sk2.to_bytes());

        // sign with the derived keys
        let rust_sig2 = sign(&rust_sk2, data);
        let py_sig2 = aug.call_method1("sign",
            PyTuple::new(py, &[py_sk2, PyBytes::new(py, data)])).unwrap();
        assert_eq!(to_bytes(py_sig2), rust_sig2.to_bytes());

        // aggregate
        let rust_agg = aggregate(&[rust_sig1, rust_sig2]);
        let py_agg = aug.call_method1("aggregate", PyTuple::new(py,
            &[PyList::new(py, &[py_sig1, py_sig2])])).unwrap();
        assert_eq!(to_bytes(py_agg), rust_agg.to_bytes());
    });

});
