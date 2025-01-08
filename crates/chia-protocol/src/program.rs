use crate::bytes::Bytes;
use chia_sha2::Sha256;
use chia_traits::chia_error::{Error, Result};
use chia_traits::Streamable;
use clvm_traits::{FromClvm, FromClvmError, ToClvm, ToClvmError};
use clvmr::allocator::NodePtr;
use clvmr::cost::Cost;
use clvmr::reduction::EvalErr;
use clvmr::run_program;
use clvmr::serde::{
    node_from_bytes, node_from_bytes_backrefs, node_to_bytes, serialized_length_from_bytes,
    serialized_length_from_bytes_trusted,
};
use clvmr::{Allocator, ChiaDialect};
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyType;
use std::io::Cursor;
use std::ops::Deref;

#[cfg_attr(feature = "py-bindings", pyclass, derive(PyStreamable))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Program(Bytes);

impl Default for Program {
    fn default() -> Self {
        Self(vec![0x80].into())
    }
}

impl Program {
    pub fn new(bytes: Bytes) -> Self {
        Self(bytes)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    pub fn into_inner(self) -> Bytes {
        self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0.into_inner()
    }

    pub fn run<A: ToClvm<Allocator>>(
        &self,
        a: &mut Allocator,
        flags: u32,
        max_cost: Cost,
        arg: &A,
    ) -> std::result::Result<(Cost, NodePtr), EvalErr> {
        let arg = arg.to_clvm(a).map_err(|_| {
            EvalErr(
                a.nil(),
                "failed to convert argument to CLVM objects".to_string(),
            )
        })?;
        let program =
            node_from_bytes_backrefs(a, self.0.as_ref()).expect("invalid SerializedProgram");
        let dialect = ChiaDialect::new(flags);
        let reduction = run_program(a, &dialect, program, arg, max_cost)?;
        Ok((reduction.0, reduction.1))
    }
}

impl From<Bytes> for Program {
    fn from(value: Bytes) -> Self {
        Self(value)
    }
}

impl From<Program> for Bytes {
    fn from(value: Program) -> Self {
        value.0
    }
}

impl From<Vec<u8>> for Program {
    fn from(value: Vec<u8>) -> Self {
        Self(Bytes::new(value))
    }
}

impl From<&[u8]> for Program {
    fn from(value: &[u8]) -> Self {
        Self(value.into())
    }
}

impl From<Program> for Vec<u8> {
    fn from(value: Program) -> Self {
        value.0.into()
    }
}

impl AsRef<[u8]> for Program {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Deref for Program {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Program {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // generate an arbitrary CLVM structure. Not likely a valid program.
        let mut items_left = 1;
        let mut total_items = 0;
        let mut buf = Vec::<u8>::with_capacity(200);

        while items_left > 0 {
            if total_items < 100 && u.ratio(1, 4).unwrap() {
                // make a pair
                buf.push(0xff);
                items_left += 2;
            } else {
                // make an atom. just single bytes for now
                buf.push(u.int_in_range(0..=0x80).unwrap());
            }
            total_items += 1;
            items_left -= 1;
        }
        Ok(Self(buf.into()))
    }
}

#[cfg(feature = "py-bindings")]
use crate::lazy_node::LazyNode;

#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;

#[cfg(feature = "py-bindings")]
use pyo3::types::{PyList, PyTuple};

#[cfg(feature = "py-bindings")]
use clvmr::allocator::SExp;

#[cfg(feature = "py-bindings")]
use pyo3::exceptions::*;

// TODO: this conversion function should probably be converted to a type holding
// the PyAny object implementing the ToClvm trait. That way, the Program::to()
// function could turn a python structure directly into bytes, without taking
// the detour via Allocator. propagating python errors through ToClvmError is a
// bit tricky though
#[cfg(feature = "py-bindings")]
fn clvm_convert(a: &mut Allocator, o: &Bound<'_, PyAny>) -> PyResult<NodePtr> {
    // None
    if o.is_none() {
        Ok(a.nil())
    // bytes
    } else if let Ok(buffer) = o.extract::<&[u8]>() {
        a.new_atom(buffer)
            .map_err(|e| PyMemoryError::new_err(e.to_string()))
    // str
    } else if let Ok(text) = o.extract::<String>() {
        a.new_atom(text.as_bytes())
            .map_err(|e| PyMemoryError::new_err(e.to_string()))
    // int
    } else if let Ok(val) = o.extract::<clvmr::number::Number>() {
        a.new_number(val)
            .map_err(|e| PyMemoryError::new_err(e.to_string()))
    // Tuple (SExp-like)
    } else if let Ok(pair) = o.downcast::<PyTuple>() {
        if pair.len() == 2 {
            let left = clvm_convert(a, &pair.get_item(0)?)?;
            let right = clvm_convert(a, &pair.get_item(1)?)?;
            a.new_pair(left, right)
                .map_err(|e| PyMemoryError::new_err(e.to_string()))
        } else {
            Err(PyValueError::new_err(format!(
                "can't cast tuple of size {}",
                pair.len()
            )))
        }
    // List
    } else if let Ok(list) = o.downcast::<PyList>() {
        let mut rev = Vec::new();
        for py_item in list.iter() {
            rev.push(py_item);
        }
        let mut ret = a.nil();
        for py_item in rev.into_iter().rev() {
            let item = clvm_convert(a, &py_item)?;
            ret = a
                .new_pair(item, ret)
                .map_err(|e| PyMemoryError::new_err(e.to_string()))?;
        }
        Ok(ret)
    // SExp (such as clvm.SExp)
    } else if let (Ok(atom), Ok(pair)) = (o.getattr("atom"), o.getattr("pair")) {
        if atom.is_none() {
            if pair.is_none() {
                Err(PyTypeError::new_err(format!("invalid SExp item {o}")))
            } else {
                let pair = pair.downcast::<PyTuple>()?;
                let left = clvm_convert(a, &pair.get_item(0)?)?;
                let right = clvm_convert(a, &pair.get_item(1)?)?;
                a.new_pair(left, right)
                    .map_err(|e| PyMemoryError::new_err(e.to_string()))
            }
        } else {
            a.new_atom(atom.extract::<&[u8]>()?)
                .map_err(|e| PyMemoryError::new_err(e.to_string()))
        }
    // Program itself. This is interpreted as a program in serialized form, and
    // just a buffer of that serialization. This is an optimization to finding
    // __bytes__() and calling it
    } else if let Ok(prg) = o.extract::<Program>() {
        a.new_atom(prg.0.as_slice())
            .map_err(|e| PyMemoryError::new_err(e.to_string()))
    // anything convertible to bytes
    } else if let Ok(fun) = o.getattr("__bytes__") {
        let bytes = fun.call0()?;
        let buffer = bytes.extract::<&[u8]>()?;
        a.new_atom(buffer)
            .map_err(|e| PyMemoryError::new_err(e.to_string()))
    } else {
        Err(PyTypeError::new_err(format!(
            "unknown parameter to run_with_cost() {o}"
        )))
    }
}

#[cfg(feature = "py-bindings")]
fn clvm_serialize(a: &mut Allocator, o: &Bound<'_, PyAny>) -> PyResult<NodePtr> {
    /*
    When passing arguments to run(), there's some special treatment, before falling
    back on the regular python -> CLVM conversion (implemented by clvm_convert
    above). This function mimics the _serialize() function in python:

       def _serialize(node: object) -> bytes:
           if isinstance(node, list):
               serialized_list = bytearray()
               for a in node:
                   serialized_list += b"\xff"
                   serialized_list += _serialize(a)
               serialized_list += b"\x80"
               return bytes(serialized_list)
           if type(node) is SerializedProgram:
               return bytes(node)
           if type(node) is Program:
               return bytes(node)
           else:
               ret: bytes = SExp.to(node).as_bin()
               return ret
    */

    // List
    if let Ok(list) = o.downcast::<PyList>() {
        let mut rev = Vec::new();
        for py_item in list.iter() {
            rev.push(py_item);
        }
        let mut ret = a.nil();
        for py_item in rev.into_iter().rev() {
            let item = clvm_serialize(a, &py_item)?;
            ret = a
                .new_pair(item, ret)
                .map_err(|e| PyMemoryError::new_err(e.to_string()))?;
        }
        Ok(ret)
    // Program itself
    } else if let Ok(prg) = o.extract::<Program>() {
        Ok(node_from_bytes_backrefs(a, prg.0.as_slice())?)
    } else {
        clvm_convert(a, o)
    }
}

#[cfg(feature = "py-bindings")]
fn to_program(py: Python<'_>, node: LazyNode) -> PyResult<Bound<'_, PyAny>> {
    let int_module = PyModule::import_bound(py, "chia.types.blockchain_format.program")?;
    let ty = int_module.getattr("Program")?;
    ty.call1((node.into_py(py),))
}

#[cfg(feature = "py-bindings")]
#[allow(clippy::needless_pass_by_value)]
#[pymethods]
impl Program {
    #[pyo3(name = "default")]
    #[staticmethod]
    fn py_default() -> Self {
        Self::default()
    }

    #[staticmethod]
    #[pyo3(name = "to")]
    fn py_to(args: &Bound<'_, PyAny>) -> PyResult<Program> {
        let mut a = Allocator::new_limited(500_000_000);
        let clvm = clvm_convert(&mut a, args)?;
        Program::from_clvm(&a, clvm)
            .map_err(|error| PyErr::new::<PyTypeError, _>(error.to_string()))
    }

    fn get_tree_hash(&self) -> crate::Bytes32 {
        clvm_utils::tree_hash_from_bytes(self.0.as_ref())
            .unwrap()
            .into()
    }

    #[staticmethod]
    fn from_program(py: Python<'_>, p: PyObject) -> PyResult<Self> {
        let buf = p.getattr(py, "__bytes__")?.call0(py)?;
        let buf = buf.extract::<&[u8]>(py)?;
        Ok(Self(buf.into()))
    }

    #[staticmethod]
    fn fromhex(h: String) -> Result<Self> {
        let s = if let Some(st) = h.strip_prefix("0x") {
            st
        } else {
            &h[..]
        };
        Self::from_bytes(hex::decode(s).map_err(|_| Error::InvalidString)?.as_slice())
    }

    fn run_mempool_with_cost<'a>(
        &self,
        py: Python<'a>,
        max_cost: u64,
        args: &Bound<'_, PyAny>,
    ) -> PyResult<(u64, Bound<'a, PyAny>)> {
        use clvmr::MEMPOOL_MODE;
        self._run(py, max_cost, MEMPOOL_MODE, args)
    }

    fn run_with_cost<'a>(
        &self,
        py: Python<'a>,
        max_cost: u64,
        args: &Bound<'_, PyAny>,
    ) -> PyResult<(u64, Bound<'a, PyAny>)> {
        self._run(py, max_cost, 0, args)
    }

    fn _run<'a>(
        &self,
        py: Python<'a>,
        max_cost: u64,
        flags: u32,
        args: &Bound<'_, PyAny>,
    ) -> PyResult<(u64, Bound<'a, PyAny>)> {
        use clvmr::reduction::Response;
        use std::rc::Rc;

        let mut a = Allocator::new_limited(500_000_000);
        // The python behavior here is a bit messy, and is best not emulated
        // on the rust side. We must be able to pass a Program as an argument,
        // and it being treated as the CLVM structure it represents. In python's
        // SerializedProgram, we have a hack where we interpret the first
        // "layer" of SerializedProgram, or lists of SerializedProgram this way.
        // But if we encounter an Optional or tuple, we defer to the clvm
        // wheel's conversion function to SExp. This level does not have any
        // special treatment for SerializedProgram (as that would cause a
        // circular dependency).
        let clvm_args = clvm_serialize(&mut a, args)?;

        let r: Response = (|| -> PyResult<Response> {
            let program = node_from_bytes_backrefs(&mut a, self.0.as_ref())?;
            let dialect = ChiaDialect::new(flags);

            Ok(py.allow_threads(|| run_program(&mut a, &dialect, program, clvm_args, max_cost)))
        })()?;
        match r {
            Ok(reduction) => {
                let val = LazyNode::new(Rc::new(a), reduction.1);
                Ok((reduction.0, to_program(py, val)?))
            }
            Err(eval_err) => {
                let blob = node_to_bytes(&a, eval_err.0).ok().map(hex::encode);
                Err(PyValueError::new_err((eval_err.1, blob)))
            }
        }
    }

    fn to_program<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        use std::rc::Rc;
        let mut a = Allocator::new_limited(500_000_000);
        let prg = node_from_bytes_backrefs(&mut a, self.0.as_ref())?;
        let prg = LazyNode::new(Rc::new(a), prg);
        to_program(py, prg)
    }

    fn uncurry<'a>(&self, py: Python<'a>) -> PyResult<(Bound<'a, PyAny>, Bound<'a, PyAny>)> {
        use clvm_utils::CurriedProgram;
        use std::rc::Rc;

        let mut a = Allocator::new_limited(500_000_000);
        let prg = node_from_bytes_backrefs(&mut a, self.0.as_ref())?;
        let Ok(uncurried) = CurriedProgram::<NodePtr, NodePtr>::from_clvm(&a, prg) else {
            let a = Rc::new(a);
            let prg = LazyNode::new(a.clone(), prg);
            let ret = a.nil();
            let ret = LazyNode::new(a, ret);
            return Ok((to_program(py, prg)?, to_program(py, ret)?));
        };

        let mut curried_args = Vec::<NodePtr>::new();
        let mut args = uncurried.args;
        loop {
            if let SExp::Atom = a.sexp(args) {
                break;
            }
            // the args of curried puzzles are in the form of:
            // (c . ((q . <arg1>) . (<rest> . ())))
            let (_, ((_, arg), (rest, ()))) =
                <(
                    clvm_traits::MatchByte<4>,
                    (clvm_traits::match_quote!(NodePtr), (NodePtr, ())),
                ) as FromClvm<Allocator>>::from_clvm(&a, args)
                .map_err(|error| PyErr::new::<PyTypeError, _>(error.to_string()))?;
            curried_args.push(arg);
            args = rest;
        }
        let mut ret = a.nil();
        for item in curried_args.into_iter().rev() {
            ret = a.new_pair(item, ret).map_err(|_e| Error::EndOfBuffer)?;
        }
        let a = Rc::new(a);
        let prg = LazyNode::new(a.clone(), uncurried.program);
        let ret = LazyNode::new(a, ret);
        Ok((to_program(py, prg)?, to_program(py, ret)?))
    }
}

impl Streamable for Program {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(&self.0);
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        out.extend_from_slice(self.0.as_ref());
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let pos = input.position();
        let buf: &[u8] = &input.get_ref()[pos as usize..];
        let len = if TRUSTED {
            serialized_length_from_bytes_trusted(buf).map_err(|_e| Error::EndOfBuffer)?
        } else {
            serialized_length_from_bytes(buf).map_err(|_e| Error::EndOfBuffer)?
        };
        if buf.len() < len as usize {
            return Err(Error::EndOfBuffer);
        }
        let program = buf[..len as usize].to_vec();
        input.set_position(pos + len);
        Ok(Program(program.into()))
    }
}

#[cfg(feature = "py-bindings")]
impl ToJsonDict for Program {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        self.0.to_json_dict(py)
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl Program {
    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(_cls: &Bound<'_, PyType>, _instance: &Self) -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "This class does not support from_parent().",
        ))
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for Program {
    fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
        let bytes = Bytes::from_json_dict(o)?;
        let len =
            serialized_length_from_bytes(bytes.as_slice()).map_err(|_e| Error::EndOfBuffer)?;
        if len as usize != bytes.len() {
            // If the bytes in the JSON string is not a valid CLVM
            // serialization, or if it has garbage at the end of the string,
            // reject it
            return Err(Error::InvalidClvm)?;
        }
        Ok(Self(bytes))
    }
}

impl FromClvm<Allocator> for Program {
    fn from_clvm(a: &Allocator, node: NodePtr) -> std::result::Result<Self, FromClvmError> {
        Ok(Self(
            node_to_bytes(a, node)
                .map_err(|error| FromClvmError::Custom(error.to_string()))?
                .into(),
        ))
    }
}

impl ToClvm<Allocator> for Program {
    fn to_clvm(&self, a: &mut Allocator) -> std::result::Result<NodePtr, ToClvmError> {
        node_from_bytes(a, self.0.as_ref()).map_err(|error| ToClvmError::Custom(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_roundtrip() {
        let a = &mut Allocator::new();
        let expected = "ff01ff02ff62ff0480";
        let expected_bytes = hex::decode(expected).unwrap();

        let ptr = node_from_bytes(a, &expected_bytes).unwrap();
        let program = Program::from_clvm(a, ptr).unwrap();

        let round_trip = program.to_clvm(a).unwrap();
        assert_eq!(expected, hex::encode(node_to_bytes(a, round_trip).unwrap()));
    }

    #[test]
    fn program_run() {
        let a = &mut Allocator::new();

        // (+ 2 5)
        let prg = Program::from_bytes(&hex::decode("ff10ff02ff0580").expect("hex::decode"))
            .expect("from_bytes");
        let (cost, result) = prg.run(a, 0, 1000, &[1300, 37]).expect("run");
        assert_eq!(cost, 869);
        assert_eq!(a.number(result), 1337.into());
    }
}
