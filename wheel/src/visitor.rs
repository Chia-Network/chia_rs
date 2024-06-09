use chia_traits::{StubBuilder, TypeStub};
use pyo3::{prelude::*, PyClass};

pub trait Visitor {
    fn visit<T: PyClass + TypeStub>(&self) -> Result<(), PyErr>;
    fn int(&self, name: &str, value: u32) -> Result<(), PyErr>;
}

impl Visitor for Bound<'_, PyModule> {
    fn visit<T: PyClass>(&self) -> Result<(), PyErr> {
        self.add_class::<T>()
    }

    fn int(&self, name: &str, value: u32) -> Result<(), PyErr> {
        self.add(name, value)
    }
}

impl Visitor for StubBuilder {
    fn visit<T: TypeStub>(&self) -> Result<(), PyErr> {
        self.stub::<T>();
        Ok(())
    }

    fn int(&self, name: &str, _value: u32) -> Result<(), PyErr> {
        self.define(name, format!("{name}: int = ..."));
        Ok(())
    }
}
