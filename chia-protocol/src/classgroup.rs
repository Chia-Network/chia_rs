use crate::streamable_struct;
use crate::Bytes100;
use chia_streamable_macro::Streamable;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct!(ClassgroupElement { data: Bytes100 });

#[cfg(feature = "py-bindings")]
#[cfg_attr(feature = "py-bindings", pymethods)]
impl ClassgroupElement {
    #[staticmethod]
    pub fn get_default_element() -> ClassgroupElement {
        let mut data = [0_u8; 100];
        data[0] = 0x08;
        ClassgroupElement { data: data.into() }
    }

    #[staticmethod]
    pub fn get_size(_constants: pyo3::PyObject) -> i32 {
        100
    }
}
