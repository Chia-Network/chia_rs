use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

pub fn py_int<'a, T: pyo3::ToPyObject>(
    py: pyo3::Python<'a>,
    py_type: &str,
    val: T,
) -> PyResult<&'a PyAny> {
    let ctx: &'a PyDict = PyDict::new(py);
    ctx.set_item("value", val.to_object(py))?;
    py.run(
        format!(
            "from chia.util.ints import {py_type}\n\
        ret = {py_type}(value)\n"
        )
        .as_str(),
        None,
        Some(ctx),
    )?;
    Ok(ctx.get_item("ret").unwrap())
}

#[macro_export]
macro_rules! convert_int {
    ($name:expr, $py:ident, $c:ident, i8) => {
        $c::py_int($py, "int8", $name)?
    };
    ($name:expr, $py:ident, $c:ident, u8) => {
        $c::py_int($py, "uint8", $name)?
    };
    ($name:expr, $py:ident, $c:ident, i16) => {
        $c::py_int($py, "int16", $name)?
    };
    ($name:expr, $py:ident, $c:ident, u16) => {
        $c::py_int($py, "uint16", $name)?
    };
    ($name:expr, $py:ident, $c:ident, i32) => {
        $c::py_int($py, "int32", $name)?
    };
    ($name:expr, $py:ident, $c:ident, u32) => {
        $c::py_int($py, "uint32", $name)?
    };
    ($name:expr, $py:ident, $c:ident, i64) => {
        $c::py_int($py, "int64", $name)?
    };
    ($name:expr, $py:ident, $c:ident, u64) => {
        $c::py_int($py, "uint64", $name)?
    };
    ($name:expr, $py:ident, $c:ident, i128) => {
        $c::py_int($py, "int128", $name)?
    };
    ($name:expr, $py:ident, $c:ident, u128) => {
        $c::py_int($py, "uint128", $name)?
    };
    ($name:expr, $py:ident, $c:ident, Option<$t:ty>) => {
        match &$name {
            Some(v) => $c::convert_int!(v, $py, $c, $t),
            None => $py.None().into_ref($py),
        }
    };
    ($name:expr, $py:ident, $c:ident, $t:ty) => {
        pyo3::IntoPy::into_py($name.clone(), $py).into_ref($py)
    };
}
