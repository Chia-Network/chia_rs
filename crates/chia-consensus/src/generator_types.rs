use chia_protocol::Program;
use chia_streamable_macro::streamable;

#[cfg(feature = "py-bindings")]
#[cfg_attr(feature = "py-bindings", pyo3::pyclass(module = "chia_rs"))]
#[streamable]
pub struct BlockGenerator {
    program: Program,
    generator_refs: Vec<Program>,

    // the heights are only used when creating new blocks, never when validating
    block_height_list: Vec<u32>,
}
