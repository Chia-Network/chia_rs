use chia_traits::{StubBuilder, TypeStub};
use pyo3::{prelude::*, PyClass};

pub trait Visitor {
    fn visit<T: PyClass + TypeStub>(&self) -> Result<(), PyErr>;
}

impl Visitor for Bound<'_, PyModule> {
    fn visit<T: PyClass>(&self) -> Result<(), PyErr> {
        self.add_class::<T>()
    }
}

impl Visitor for StubBuilder {
    fn visit<T: TypeStub>(&self) -> Result<(), PyErr> {
        self.stub::<T>();
        Ok(())
    }
}
