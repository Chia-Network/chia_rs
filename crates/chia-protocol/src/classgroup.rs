use crate::Bytes100;
use chia_streamable_macro::streamable;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[streamable]
#[derive(Copy)]
pub struct ClassgroupElement {
    data: Bytes100,
}

impl Default for ClassgroupElement {
    fn default() -> Self {
        let mut data = [0_u8; 100];
        data[0] = 0x08;
        Self { data: data.into() }
    }
}

impl ClassgroupElement {
    pub const SIZE: usize = 100;
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl ClassgroupElement {
    #[staticmethod]
    pub fn create(bytes: &[u8]) -> ClassgroupElement {
        if bytes.len() == 100 {
            ClassgroupElement {
                data: bytes.try_into().unwrap(),
            }
        } else {
            assert!(bytes.len() < 100);
            let mut data = [0_u8; 100];
            data[..bytes.len()].copy_from_slice(bytes);
            ClassgroupElement { data: data.into() }
        }
    }

    #[staticmethod]
    #[pyo3(name = "get_default_element")]
    pub fn py_get_default_element() -> ClassgroupElement {
        Self::default()
    }

    #[staticmethod]
    #[pyo3(name = "get_size")]
    pub fn py_get_size() -> i32 {
        Self::SIZE as i32
    }
}
