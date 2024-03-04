use crate::streamable_struct;
use crate::Bytes100;
use chia_streamable_macro::Streamable;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct!(ClassgroupElement { data: Bytes100 });

impl ClassgroupElement {
    pub fn get_default_element() -> ClassgroupElement {
        let mut data = [0_u8; 100];
        data[0] = 0x08;
        ClassgroupElement { data: data.into() }
    }

    pub fn get_size() -> i32 {
        100
    }
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
        Self::get_default_element()
    }

    #[staticmethod]
    #[pyo3(name = "get_size")]
    pub fn py_get_size() -> i32 {
        Self::get_size()
    }
}
