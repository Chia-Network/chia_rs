use crate::bytes::Bytes;
use chia_traits::chia_error::{Error, Result};
use chia_traits::Streamable;
use clvm_traits::{FromClvmError, FromNodePtr, ToClvmError, ToNodePtr};
use clvmr::allocator::NodePtr;
use clvmr::serde::{node_from_bytes, node_to_bytes, serialized_length_from_bytes};
use clvmr::Allocator;
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[cfg_attr(feature = "py-bindings", pyclass, derive(PyStreamable))]
#[derive(Hash, Debug, Clone, Eq, PartialEq)]
pub struct Program(Bytes);

#[cfg(fuzzing)]
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

impl Program {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl Default for Program {
    fn default() -> Self {
        Self(vec![0x80_u8].into())
    }
}

#[cfg(feature = "py-bindings")]
use crate::lazy_node::LazyNode;

#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
use pyo3::types::{PyList, PyTuple};

#[cfg(feature = "py-bindings")]
use clvmr::serde::node_from_bytes_backrefs;

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
fn clvm_convert(a: &mut Allocator, o: &PyAny) -> PyResult<NodePtr> {
    // None
    if o.is_none() {
        Ok(a.null())
    // Program itself
    } else if let Ok(prg) = o.extract::<Program>() {
        Ok(node_from_bytes_backrefs(a, prg.0.as_slice())?)
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
        if pair.len() != 2 {
            Err(PyValueError::new_err(format!(
                "can't cast tuple of size {}",
                pair.len()
            )))
        } else {
            let left = clvm_convert(a, pair.get_item(0)?)?;
            let right = clvm_convert(a, pair.get_item(1)?)?;
            a.new_pair(left, right)
                .map_err(|e| PyMemoryError::new_err(e.to_string()))
        }
    // List
    } else if let Ok(list) = o.downcast::<PyList>() {
        let mut rev = Vec::<&PyAny>::new();
        for py_item in list.iter() {
            rev.push(py_item);
        }
        let mut ret = a.null();
        for py_item in rev.into_iter().rev() {
            let item = clvm_convert(a, py_item)?;
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
                let left = clvm_convert(a, &pair[0])?;
                let right = clvm_convert(a, &pair[1])?;
                a.new_pair(left, right)
                    .map_err(|e| PyMemoryError::new_err(e.to_string()))
            }
        } else {
            a.new_atom(atom.extract::<&[u8]>()?)
                .map_err(|e| PyMemoryError::new_err(e.to_string()))
        }
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
fn to_program(py: Python<'_>, node: LazyNode) -> PyResult<&PyAny> {
    use pyo3::types::PyDict;
    let ctx: &PyDict = PyDict::new(py);
    ctx.set_item("node", node)?;
    py.run(
        "from chia.types.blockchain_format.program import Program\n\
        ret = Program(node)\n",
        None,
        Some(ctx),
    )?;
    Ok(ctx.get_item("ret").unwrap())
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl Program {
    #[pyo3(name = "default")]
    #[staticmethod]
    fn py_default() -> Self {
        Self::default()
    }

    #[staticmethod]
    #[pyo3(name = "to")]
    fn py_to(args: &PyAny) -> PyResult<Program> {
        let mut a = Allocator::new_limited(500000000, 62500000, 62500000);
        let clvm = clvm_convert(&mut a, args)?;
        Ok(Program::from_node_ptr(&a, clvm)?)
    }

    fn get_tree_hash(&self) -> crate::Bytes32 {
        let mut cursor = Cursor::new(self.0.as_ref());
        clvmr::serde::tree_hash_from_stream(&mut cursor)
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
        args: &PyAny,
    ) -> PyResult<(u64, &'a PyAny)> {
        use clvmr::MEMPOOL_MODE;
        self._run(py, max_cost, MEMPOOL_MODE, args)
    }

    fn run_with_cost<'a>(
        &self,
        py: Python<'a>,
        max_cost: u64,
        args: &PyAny,
    ) -> PyResult<(u64, &'a PyAny)> {
        self._run(py, max_cost, 0, args)
    }

    fn _run<'a>(
        &self,
        py: Python<'a>,
        max_cost: u64,
        flags: u32,
        args: &PyAny,
    ) -> PyResult<(u64, &'a PyAny)> {
        use clvmr::reduction::Response;
        use clvmr::run_program;
        use clvmr::ChiaDialect;
        use std::rc::Rc;

        let mut a = Allocator::new_limited(500000000, 62500000, 62500000);
        let clvm_args = clvm_convert(&mut a, args)?;

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
            Err(eval_err) => Err(PyValueError::new_err(eval_err.to_string())),
        }
    }

    fn to_program<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        use std::rc::Rc;
        let mut a = Allocator::new_limited(500000000, 62500000, 62500000);
        let prg = node_from_bytes_backrefs(&mut a, self.0.as_ref())?;
        let prg = LazyNode::new(Rc::new(a), prg);
        to_program(py, prg)
    }

    fn uncurry<'a>(&self, py: Python<'a>) -> PyResult<(&'a PyAny, &'a PyAny)> {
        use clvm_utils::CurriedProgram;
        use std::rc::Rc;

        let mut a = Allocator::new_limited(500000000, 62500000, 62500000);
        let prg = node_from_bytes_backrefs(&mut a, self.0.as_ref())?;
        let Ok(uncurried) = CurriedProgram::<NodePtr, NodePtr>::from_node_ptr(&a, prg) else {
            let a = Rc::new(a);
            let prg = LazyNode::new(a.clone(), prg);
            let ret = a.null();
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
            let (_, ((_, arg), (rest, _))) =
                <(
                    clvm_traits::MatchByte<4>,
                    (clvm_traits::match_quote!(NodePtr), (NodePtr, ())),
                ) as clvm_traits::FromNodePtr>::from_node_ptr(&a, args)?;
            curried_args.push(arg);
            args = rest;
        }
        let mut ret = a.null();
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

    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let pos = input.position();
        let buf: &[u8] = &input.get_ref()[pos as usize..];
        let len = serialized_length_from_bytes(buf).map_err(|_e| Error::EndOfBuffer)?;
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
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        self.0.to_json_dict(py)
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for Program {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
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

impl FromNodePtr for Program {
    fn from_node_ptr(a: &Allocator, node: NodePtr) -> std::result::Result<Self, FromClvmError> {
        Ok(Self(
            node_to_bytes(a, node)
                .map_err(|error| FromClvmError::Custom(error.to_string()))?
                .into(),
        ))
    }
}

impl ToNodePtr for Program {
    fn to_node_ptr(&self, a: &mut Allocator) -> std::result::Result<NodePtr, ToClvmError> {
        node_from_bytes(a, self.0.as_ref()).map_err(|error| ToClvmError::Custom(error.to_string()))
    }
}

impl AsRef<[u8]> for Program {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
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
        let program = Program::from_node_ptr(a, ptr).unwrap();

        let round_trip = program.to_node_ptr(a).unwrap();
        assert_eq!(expected, hex::encode(node_to_bytes(a, round_trip).unwrap()));
    }
}
