use std::io::Cursor;

#[cfg(feature = "py-bindings")]
use pyo3::FromPyObject;

use chia_sha2::Sha256;
use chia_traits::{read_bytes, Error, Result, Streamable};

/// This wrapper allows storing two optionals using a single byte to indicate
/// which ones are set. The regular Option is serialized using a whole byte per
/// field, just to use 1 bit. Using TwoOption is more space efficient and is
/// also backwards compatible with a single optional, as long as the second
/// optional isn't set.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "py-bindings", derive(FromPyObject))]
#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub struct TwoOption<T, U>(pub Option<T>, pub Option<U>);

impl<T, U> Streamable for TwoOption<T, U>
where
    T: Streamable,
    U: Streamable,
{
    fn update_digest(&self, digest: &mut Sha256) {
        let bits: u8 = if self.0.is_some() { 1 } else { 0 } | if self.1.is_some() { 2 } else { 0 };

        digest.update([bits]);
        if let Some(o) = &self.0 {
            o.update_digest(digest);
        }
        if let Some(o) = &self.1 {
            o.update_digest(digest);
        }
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        let bits: u8 = if self.0.is_some() { 1 } else { 0 } | if self.1.is_some() { 2 } else { 0 };

        out.extend_from_slice(&[bits]);
        if let Some(o) = &self.0 {
            o.stream(out)?;
        }
        if let Some(o) = &self.1 {
            o.stream(out)?;
        }
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let bits = read_bytes(input, 1)?[0];
        if (bits & 0b1111_1100) != 0 {
            return Err(Error::InvalidOptional);
        }
        let first = if (bits & 1) != 0 {
            Some(T::parse::<TRUSTED>(input)?)
        } else {
            None
        };
        let second = if (bits & 2) != 0 {
            Some(U::parse::<TRUSTED>(input)?)
        } else {
            None
        };
        Ok(Self(first, second))
    }
}

#[cfg(feature = "py-bindings")]
mod pybindings {
    use super::*;
    use chia_traits::ChiaToPython;
    use pyo3::exceptions::PyValueError;
    use pyo3::types::{PyAnyMethods, PyList, PyListMethods, PyTuple};
    use pyo3::{Bound, PyAny, PyObject, PyResult, Python};

    use chia_traits::{FromJsonDict, ToJsonDict};

    impl<T, U> ToJsonDict for TwoOption<T, U>
    where
        T: ToJsonDict,
        U: ToJsonDict,
    {
        fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
            let list = PyList::empty(py);
            list.append(self.0.to_json_dict(py)?)?;
            list.append(self.1.to_json_dict(py)?)?;
            Ok(list.into())
        }
    }

    impl<T, U> FromJsonDict for TwoOption<T, U>
    where
        T: FromJsonDict,
        U: FromJsonDict,
    {
        fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
            if o.len()? != 2 {
                return Err(PyValueError::new_err(format!(
                    "expected 2 elements, got {}",
                    o.len()?
                )));
            }
            Ok(TwoOption(
                <Option<T> as FromJsonDict>::from_json_dict(&o.get_item(0)?)?,
                <Option<U> as FromJsonDict>::from_json_dict(&o.get_item(1)?)?,
            ))
        }
    }

    impl<T: ChiaToPython, U: ChiaToPython> ChiaToPython for TwoOption<T, U> {
        fn to_python<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
            Ok(PyTuple::new(py, [self.0.to_python(py)?, self.1.to_python(py)?])?.into_any())
        }
    }
}
