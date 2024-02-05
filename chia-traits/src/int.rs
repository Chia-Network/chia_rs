use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyList, PyString, PyTuple};
use pyo3::PyResult;

/// A custom to-python conversion trait that turns primitive integer types into
/// the chia-blockchain fixed-width integer types (uint8, int8, etc.)
pub trait ChiaToPython {
    fn to_python<'a>(&self, py: pyo3::Python<'a>) -> pyo3::PyResult<&'a pyo3::types::PyAny>;
}

macro_rules! primitive_int {
    ($t:ty, $name:expr) => {
        impl ChiaToPython for $t {
            fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
                let int_module = PyModule::import(py, "chia.util.ints")?;
                let ty = int_module.getattr($name)?;
                ty.call1((self.into_py(py),))
            }
        }
    };
}

primitive_int!(i8, "int8");
primitive_int!(u8, "uint8");
primitive_int!(i16, "int16");
primitive_int!(u16, "uint16");
primitive_int!(i32, "int32");
primitive_int!(u32, "uint32");
primitive_int!(i64, "int64");
primitive_int!(u64, "uint64");
primitive_int!(i128, "int128");
primitive_int!(u128, "uint128");

impl<T: ChiaToPython> ChiaToPython for Option<T> {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        match &self {
            Some(v) => v.to_python(py),
            None => Ok(py.None().into_ref(py)),
        }
    }
}

impl<T: ChiaToPython> ChiaToPython for Vec<T> {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        let ret = PyList::empty(py);
        for v in self {
            ret.append(v.to_python(py)?)?;
        }
        Ok(ret)
    }
}

impl ChiaToPython for bool {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        Ok(PyBool::new(py, *self))
    }
}

impl ChiaToPython for String {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        Ok(PyString::new(py, self.as_str()))
    }
}

impl<T: ChiaToPython, U: ChiaToPython> ChiaToPython for (T, U) {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        Ok(PyTuple::new(
            py,
            [self.0.to_python(py)?, self.1.to_python(py)?],
        ))
    }
}

impl<T: ChiaToPython, U: ChiaToPython, V: ChiaToPython> ChiaToPython for (T, U, V) {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        Ok(PyTuple::new(
            py,
            [
                self.0.to_python(py)?,
                self.1.to_python(py)?,
                self.2.to_python(py)?,
            ],
        ))
    }
}
