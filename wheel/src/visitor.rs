use chia_traits::{FunctionBuilder, Int, StubBuilder, TypeStub};
use pyo3::{prelude::*, PyClass};

pub trait Visitor {
    fn visit<T: PyClass + TypeStub>(&self);
    fn int(&self, name: &str, value: u32);
    fn function<R: TypeStub>(
        &self,
        name: &str,
        bind: impl FnOnce(&Bound<'_, PyModule>) -> PyResult<()>,
        stub: impl FnOnce(FunctionBuilder<'_, R>) -> FunctionBuilder<'_, R>,
    );
}

impl Visitor for Bound<'_, PyModule> {
    fn visit<T: PyClass>(&self) {
        self.add_class::<T>().unwrap();
    }

    fn int(&self, name: &str, value: u32) {
        self.add(name, value).unwrap();
    }

    fn function<R: TypeStub>(
        &self,
        _name: &str,
        bind: impl FnOnce(&Bound<'_, PyModule>) -> PyResult<()>,
        _stub: impl FnOnce(FunctionBuilder<'_, R>) -> FunctionBuilder<'_, R>,
    ) {
        bind(self).unwrap();
    }
}

impl Visitor for StubBuilder {
    fn visit<T: TypeStub>(&self) {
        self.stub::<T>();
    }

    fn int(&self, name: &str, _value: u32) {
        self.constant::<Int>(name);
    }

    fn function<R: TypeStub>(
        &self,
        name: &str,
        _bind: impl FnOnce(&Bound<'_, PyModule>) -> PyResult<()>,
        stub: impl FnOnce(FunctionBuilder<'_, R>) -> FunctionBuilder<'_, R>,
    ) {
        stub(self.function(name)).generate();
    }
}
