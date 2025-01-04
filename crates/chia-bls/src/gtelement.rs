use blst::*;
use chia_sha2::Sha256;
use chia_traits::chia_error::Result;
use chia_traits::{read_bytes, Streamable};
#[cfg(feature = "py-bindings")]
use pyo3::exceptions::PyNotImplementedError;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyType;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ops::{Mul, MulAssign};

#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass,
    derive(chia_py_streamable_macro::PyStreamable)
)]
#[derive(Clone)]
pub struct GTElement(pub(crate) blst_fp12);

impl GTElement {
    const SIZE: usize = std::mem::size_of::<blst_fp12>();

    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Self {
        let gt = unsafe {
            let mut gt = MaybeUninit::<blst_fp12>::uninit();
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), gt.as_mut_ptr().cast::<u8>(), Self::SIZE);
            gt.assume_init()
        };
        Self(gt)
    }

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        unsafe {
            let mut bytes = MaybeUninit::<[u8; Self::SIZE]>::uninit();
            let buf: *const blst_fp12 = &self.0;
            std::ptr::copy_nonoverlapping(
                buf.cast::<u8>(),
                bytes.as_mut_ptr().cast::<u8>(),
                Self::SIZE,
            );
            bytes.assume_init()
        }
    }
}

impl PartialEq for GTElement {
    fn eq(&self, other: &Self) -> bool {
        unsafe { blst_fp12_is_equal(&self.0, &other.0) }
    }
}
impl Eq for GTElement {}

impl Hash for GTElement {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.to_bytes());
    }
}

impl MulAssign<&GTElement> for GTElement {
    fn mul_assign(&mut self, rhs: &GTElement) {
        unsafe {
            blst_fp12_mul(&mut self.0, &self.0, &rhs.0);
        }
    }
}

impl Mul<&GTElement> for &GTElement {
    type Output = GTElement;
    fn mul(self, rhs: &GTElement) -> GTElement {
        let gt = unsafe {
            let mut gt = MaybeUninit::<blst_fp12>::uninit();
            blst_fp12_mul(gt.as_mut_ptr(), &self.0, &rhs.0);
            gt.assume_init()
        };
        GTElement(gt)
    }
}

impl fmt::Debug for GTElement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(format_args!(
            "<GTElement {}>",
            &hex::encode(self.to_bytes())
        ))
    }
}

impl Streamable for GTElement {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(self.to_bytes());
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        out.extend_from_slice(&self.to_bytes());
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        Ok(GTElement::from_bytes(
            read_bytes(input, Self::SIZE)?.try_into().unwrap(),
        ))
    }
}

#[cfg(feature = "py-bindings")]
#[pyo3::pymethods]
impl GTElement {
    #[classattr]
    #[pyo3(name = "SIZE")]
    pub const PY_SIZE: usize = Self::SIZE;

    pub fn __str__(&self) -> String {
        hex::encode(self.to_bytes())
    }

    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(_cls: &Bound<'_, PyType>, _instance: &Self) -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "GTElement does not support from_parent().",
        ))
    }

    #[must_use]
    pub fn __mul__(&self, rhs: &Self) -> Self {
        let mut ret = self.clone();
        ret *= rhs;
        ret
    }

    pub fn __imul__(&mut self, rhs: &Self) {
        *self *= rhs;
    }
}

#[cfg(feature = "py-bindings")]
mod pybindings {
    use super::*;

    use crate::parse_hex::parse_hex_string;
    use chia_traits::{FromJsonDict, ToJsonDict};

    impl ToJsonDict for GTElement {
        fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
            let bytes = self.to_bytes();
            Ok(("0x".to_string() + &hex::encode(bytes))
                .into_pyobject(py)?
                .into_any()
                .unbind())
        }
    }

    impl FromJsonDict for GTElement {
        fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
            Ok(Self::from_bytes(
                parse_hex_string(o, Self::SIZE, "GTElement")?
                    .as_slice()
                    .try_into()
                    .unwrap(),
            ))
        }
    }
}
