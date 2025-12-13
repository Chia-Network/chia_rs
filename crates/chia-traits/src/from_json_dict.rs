use pyo3::Bound;
use pyo3::PyAny;
use pyo3::PyResult;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyAnyMethods;

pub trait FromJsonDict {
    fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self>
    where
        Self: Sized;
}

impl<T> FromJsonDict for Option<T>
where
    T: FromJsonDict,
{
    fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
        if o.is_none() {
            return Ok(None);
        }
        Ok(Some(<T as FromJsonDict>::from_json_dict(o)?))
    }
}

macro_rules! from_json_primitive {
    ($t:ty) => {
        impl $crate::from_json_dict::FromJsonDict for $t {
            fn from_json_dict(o: &Bound<'_, PyAny>) -> pyo3::PyResult<Self> {
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
    fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut ret = Vec::<T>::new();
        for v in o.try_iter()? {
            ret.push(<T as FromJsonDict>::from_json_dict(&v?)?);
        }
        Ok(ret)
    }
}

impl<T, U> FromJsonDict for (T, U)
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
        Ok((
            <T as FromJsonDict>::from_json_dict(&o.get_item(0)?)?,
            <U as FromJsonDict>::from_json_dict(&o.get_item(1)?)?,
        ))
    }
}

impl<T, U, V> FromJsonDict for (T, U, V)
where
    T: FromJsonDict,
    U: FromJsonDict,
    V: FromJsonDict,
{
    fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
        if o.len()? != 3 {
            return Err(PyValueError::new_err(format!(
                "expected 3 elements, got {}",
                o.len()?
            )));
        }
        Ok((
            <T as FromJsonDict>::from_json_dict(&o.get_item(0)?)?,
            <U as FromJsonDict>::from_json_dict(&o.get_item(1)?)?,
            <V as FromJsonDict>::from_json_dict(&o.get_item(2)?)?,
        ))
    }
}

impl<T, const N: usize> FromJsonDict for [T; N]
where
    T: FromJsonDict + std::marker::Copy + Default,
{
    fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
        if o.len()? != N {
            return Err(PyValueError::new_err(format!(
                "expected {N} elements, got {}",
                o.len()?
            )));
        }

        let mut ret = [<T as Default>::default(); N];
        for (i, v) in &mut ret.iter_mut().enumerate() {
            *v = <T as FromJsonDict>::from_json_dict(&o.get_item(i)?)?;
        }
        Ok(ret)
    }
}
