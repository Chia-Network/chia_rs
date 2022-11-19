use pyo3::exceptions::PyValueError;
use pyo3::PyAny;
use pyo3::PyResult;

use crate::bytes::{Bytes, BytesImpl};
use hex::FromHex;
use std::convert::TryInto;

pub trait FromJsonDict {
    fn from_json_dict(o: &PyAny) -> PyResult<Self>
    where
        Self: Sized;
}

impl<const N: usize> FromJsonDict for BytesImpl<N> {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        let s: String = o.extract()?;
        if !s.starts_with("0x") {
            return Err(PyValueError::new_err(
                "bytes object is expected to start with 0x",
            ));
        }
        let s = &s[2..];
        let buf = match Vec::from_hex(s) {
            Err(_) => {
                return Err(PyValueError::new_err("invalid hex"));
            }
            Ok(v) => v,
        };
        if buf.len() != N {
            return Err(PyValueError::new_err(format!(
                "invalid length {} expected {}",
                buf.len(),
                N
            )));
        }
        Ok(buf.try_into()?)
    }
}

impl FromJsonDict for Bytes {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        let s: String = o.extract()?;
        if !s.starts_with("0x") {
            return Err(PyValueError::new_err(
                "bytes object is expected to start with 0x",
            ));
        }
        let s = &s[2..];
        let buf = match Vec::from_hex(s) {
            Err(_) => {
                return Err(PyValueError::new_err("invalid hex"));
            }
            Ok(v) => v,
        };
        Ok(buf.into())
    }
}

impl<T> FromJsonDict for Option<T>
where
    T: FromJsonDict,
{
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        if o.is_none() {
            return Ok(None);
        }
        Ok(Some(<T as FromJsonDict>::from_json_dict(o)?))
    }
}

macro_rules! from_json_primitive {
    ($t:ty) => {
        impl FromJsonDict for $t {
            fn from_json_dict(o: &PyAny) -> PyResult<Self> {
                o.extract()
            }
        }
    };
}

from_json_primitive!(bool);
from_json_primitive!(u8);
from_json_primitive!(i8);
from_json_primitive!(u16);
from_json_primitive!(i16);
from_json_primitive!(u32);
from_json_primitive!(i32);
from_json_primitive!(u64);
from_json_primitive!(i64);
from_json_primitive!(u128);
from_json_primitive!(i128);
from_json_primitive!(String);

impl<T> FromJsonDict for Vec<T>
where
    T: FromJsonDict,
{
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        let mut ret = Vec::<T>::new();
        for v in o.iter()? {
            ret.push(<T as FromJsonDict>::from_json_dict(v?)?);
        }
        Ok(ret)
    }
}

impl<T, U> FromJsonDict for (T, U)
where
    T: FromJsonDict,
    U: FromJsonDict,
{
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        if o.len()? != 2 {
            return Err(PyValueError::new_err(format!(
                "expected 2 elements, got {}",
                o.len()?
            )));
        }
        Ok((
            <T as FromJsonDict>::from_json_dict(o.get_item(0)?)?,
            <U as FromJsonDict>::from_json_dict(o.get_item(1)?)?,
        ))
    }
}

impl<T, U, V> FromJsonDict for (T, U, V)
where
    T: FromJsonDict,
    U: FromJsonDict,
    V: FromJsonDict,
{
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        if o.len()? != 3 {
            return Err(PyValueError::new_err(format!(
                "expected 3 elements, got {}",
                o.len()?
            )));
        }
        Ok((
            <T as FromJsonDict>::from_json_dict(o.get_item(0)?)?,
            <U as FromJsonDict>::from_json_dict(o.get_item(1)?)?,
            <V as FromJsonDict>::from_json_dict(o.get_item(2)?)?,
        ))
    }
}
