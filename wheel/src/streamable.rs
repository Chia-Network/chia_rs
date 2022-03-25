use serde::{Deserialize, Serialize};

use chia::streamable::deserialize::ChiaDeserializer;
use chia::streamable::serialize::ChiaSerializer;

use pyo3::prelude::*;

pub fn py_from_bytes<'a, T: Deserialize<'a>>(blob: &'a [u8]) -> PyResult<T> {
    let mut de = ChiaDeserializer::from_slice(blob)?;
    T::deserialize(&mut de).map_err(|e| e.into())
}

pub fn py_parse_rust<'a, T: Deserialize<'a>>(blob: &'a [u8]) -> PyResult<(T, u32)> {
    let mut de = ChiaDeserializer::from_slice(blob)?;
    T::deserialize(&mut de)
        .map_err(|e| e.into())
        .map(|v| (v, de.pos()))
}

pub fn py_to_bytes<T: Serialize>(v: &T) -> PyResult<Vec<u8>> {
    let mut writer = Vec::<u8>::new();
    let mut ser = ChiaSerializer::new(&mut writer);
    serde::Serialize::serialize(v, &mut ser)?;
    Ok(writer)
}
