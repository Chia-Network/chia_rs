use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

pub fn parse_hex_string(o: &Bound<'_, PyAny>, len: usize, name: &str) -> PyResult<Vec<u8>> {
    if let Ok(s) = o.extract::<String>() {
        let s = if let Some(st) = s.strip_prefix("0x") {
            st
        } else {
            &s[..]
        };
        let buf = match hex::decode(s) {
            Err(_) => {
                return Err(PyValueError::new_err("invalid hex"));
            }
            Ok(v) => v,
        };
        if buf.len() == len {
            Ok(buf)
        } else {
            Err(PyValueError::new_err(format!(
                "{}, invalid length {} expected {}",
                name,
                buf.len(),
                len
            )))
        }
    } else if let Ok(buf) = o.extract::<Vec<u8>>() {
        if buf.len() == len {
            Ok(buf)
        } else {
            Err(PyValueError::new_err(format!(
                "{}, invalid length {} expected {}",
                name,
                buf.len(),
                len
            )))
        }
    } else {
        Err(PyTypeError::new_err(format!(
            "invalid input type for {name}"
        )))
    }
}
