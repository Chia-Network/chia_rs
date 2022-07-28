use chia::bytes::{Bytes, BytesImpl};
use pyo3::prelude::*;
use pyo3::types::PyList;

pub trait ToJsonDict {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject>;
}

impl ToJsonDict for u32 {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        Ok(self.to_object(py))
    }
}

impl ToJsonDict for u64 {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        Ok(self.to_object(py))
    }
}

impl<const N: usize> ToJsonDict for BytesImpl<N> {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        Ok(format!("0x{}", self).to_object(py))
    }
}

impl ToJsonDict for Bytes {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        Ok(format!("0x{}", self).to_object(py))
    }
}

impl<T: ToJsonDict> ToJsonDict for Vec<T> {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        let list = PyList::empty(py);
        for v in self {
            list.append(v.to_json_dict(py)?)?;
        }
        Ok(list.into())
    }
}

impl<T: ToJsonDict> ToJsonDict for Option<T> {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        match self {
            None => Ok(py.None()),
            Some(v) => Ok(v.to_json_dict(py)?),
        }
    }
}

// if we need more of these, we should probably make a macro
impl<T: ToJsonDict, U: ToJsonDict> ToJsonDict for (T, U) {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        let list = PyList::empty(py);
        list.append(self.0.to_json_dict(py)?)?;
        list.append(self.1.to_json_dict(py)?)?;
        Ok(list.into())
    }
}

impl<T: ToJsonDict, U: ToJsonDict, W: ToJsonDict> ToJsonDict for (T, U, W) {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        let list = PyList::empty(py);
        list.append(self.0.to_json_dict(py)?)?;
        list.append(self.1.to_json_dict(py)?)?;
        list.append(self.2.to_json_dict(py)?)?;
        Ok(list.into())
    }
}
