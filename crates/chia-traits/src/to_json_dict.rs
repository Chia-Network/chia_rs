use pyo3::prelude::*;
use pyo3::types::PyList;

pub trait ToJsonDict {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject>;
}

macro_rules! to_json_primitive {
    ($t:ty) => {
        impl $crate::to_json_dict::ToJsonDict for $t {
            fn to_json_dict(&self, py: Python<'_>) -> pyo3::PyResult<PyObject> {
                Ok(self.to_object(py))
            }
        }
    };
}

to_json_primitive!(bool);
to_json_primitive!(u8);
to_json_primitive!(i8);
to_json_primitive!(u16);
to_json_primitive!(i16);
to_json_primitive!(u32);
to_json_primitive!(i32);
to_json_primitive!(u64);
to_json_primitive!(i64);
to_json_primitive!(u128);
to_json_primitive!(i128);
to_json_primitive!(String);

impl<T: ToJsonDict> ToJsonDict for Vec<T> {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let list = PyList::empty_bound(py);
        for v in self {
            list.append(v.to_json_dict(py)?)?;
        }
        Ok(list.into())
    }
}

impl<T: ToJsonDict> ToJsonDict for Option<T> {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        match self {
            None => Ok(py.None()),
            Some(v) => Ok(v.to_json_dict(py)?),
        }
    }
}

// if we need more of these, we should probably make a macro
impl<T: ToJsonDict, U: ToJsonDict> ToJsonDict for (T, U) {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let list = PyList::empty_bound(py);
        list.append(self.0.to_json_dict(py)?)?;
        list.append(self.1.to_json_dict(py)?)?;
        Ok(list.into())
    }
}

impl<T: ToJsonDict, U: ToJsonDict, W: ToJsonDict> ToJsonDict for (T, U, W) {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let list = PyList::empty_bound(py);
        list.append(self.0.to_json_dict(py)?)?;
        list.append(self.1.to_json_dict(py)?)?;
        list.append(self.2.to_json_dict(py)?)?;
        Ok(list.into())
    }
}
