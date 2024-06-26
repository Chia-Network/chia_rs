use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Cursor;

use chia_protocol::{HeaderBlock, SubEpochChallengeSegment, SubEpochData, WeightProof};
use chia_traits::{FromJsonDict, Streamable, ToJsonDict};
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use sha2::{Digest, Sha256};

#[pyclass(frozen, get_all, name = "WeightProof")]
#[derive(Debug, Clone)]
pub struct PyWeightProof {
    pub sub_epochs: Py<PyList>,
    pub sub_epoch_segments: Py<PyList>,
    pub recent_chain_data: Py<PyList>,
}

impl Streamable for PyWeightProof {
    fn parse<const TRUSTED: bool>(input: &mut std::io::Cursor<&[u8]>) -> chia_traits::Result<Self>
    where
        Self: Sized,
    {
        let wp = WeightProof::parse::<TRUSTED>(input)?;
        Ok(Python::with_gil(|py| {
            Self::py_new(
                py,
                wp.sub_epochs,
                wp.sub_epoch_segments,
                wp.recent_chain_data,
            )
        }))
    }

    fn update_digest(&self, digest: &mut sha2::Sha256) {
        Python::with_gil(|py| {
            let wp = self.to_rust(py).unwrap();
            wp.update_digest(digest);
        });
    }

    fn stream(&self, out: &mut Vec<u8>) -> chia_traits::Result<()> {
        Python::with_gil(|py| {
            let wp = self.to_rust(py).unwrap();
            wp.stream(out)
        })
    }
}

impl PyWeightProof {
    pub fn to_rust(&self, py: Python<'_>) -> PyResult<WeightProof> {
        let list = self.sub_epochs.bind(py);
        let mut sub_epochs = Vec::with_capacity(list.len());
        for item in list.iter() {
            sub_epochs.push(item.extract::<SubEpochData>()?);
        }

        let list = self.sub_epoch_segments.bind(py);
        let mut sub_epoch_segments = Vec::with_capacity(list.len());
        for item in list.iter() {
            sub_epoch_segments.push(item.extract::<SubEpochChallengeSegment>()?);
        }

        let list = self.recent_chain_data.bind(py);
        let mut recent_chain_data = Vec::with_capacity(list.len());
        for item in list.iter() {
            recent_chain_data.push(item.extract::<HeaderBlock>()?);
        }

        Ok(WeightProof {
            sub_epochs,
            sub_epoch_segments,
            recent_chain_data,
        })
    }
}

#[pymethods]
impl PyWeightProof {
    #[new]
    #[pyo3(signature = (sub_epochs, sub_epoch_segments, recent_chain_data))]
    pub fn py_new(
        py: Python<'_>,
        sub_epochs: Vec<SubEpochData>,
        sub_epoch_segments: Vec<SubEpochChallengeSegment>,
        recent_chain_data: Vec<HeaderBlock>,
    ) -> Self {
        let sub_epochs = PyList::new_bound(py, sub_epochs).into();
        let sub_epoch_segments = PyList::new_bound(py, sub_epoch_segments).into();
        let recent_chain_data = PyList::new_bound(py, recent_chain_data).into();

        Self {
            sub_epochs,
            sub_epoch_segments,
            recent_chain_data,
        }
    }

    #[staticmethod]
    #[pyo3(signature=(json_dict))]
    pub fn from_json_dict(json_dict: &Bound<'_, PyAny>) -> PyResult<Self> {
        <Self as FromJsonDict>::from_json_dict(json_dict)
    }

    pub fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        ToJsonDict::to_json_dict(self, py)
    }

    #[pyo3(signature = (**kwargs))]
    fn replace(&self, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let mut ret = self.clone();
        if let Some(kwargs) = kwargs {
            let iter = kwargs.iter();
            for (field, value) in iter {
                let field = field.extract::<String>()?;
                match field.as_str() {
                    "sub_epochs" => ret.sub_epochs = value.extract()?,
                    "sub_epoch_segments" => ret.sub_epoch_segments = value.extract()?,
                    "recent_chain_data" => ret.recent_chain_data = value.extract()?,
                    _ => {
                        return Err(PyKeyError::new_err(format!("unknown field {field}")));
                    }
                }
            }
        }
        Ok(ret)
    }

    fn __repr__(&self) -> String {
        format!("{self:?}")
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<u64> {
        let mut hasher = DefaultHasher::new();
        Hash::hash(&self.to_rust(py)?, &mut hasher);
        Ok(Hasher::finish(&hasher))
    }

    #[staticmethod]
    #[pyo3(name = "from_bytes")]
    pub fn py_from_bytes(blob: PyBuffer<u8>) -> PyResult<Self> {
        assert!(
            blob.is_c_contiguous(),
            "from_bytes() must be called with a contiguous buffer"
        );
        let slice =
            unsafe { std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes()) };
        Self::from_bytes(slice).map_err(PyErr::from)
    }

    #[staticmethod]
    #[pyo3(name = "from_bytes_unchecked")]
    pub fn py_from_bytes_unchecked(blob: PyBuffer<u8>) -> PyResult<Self> {
        assert!(
            blob.is_c_contiguous(),
            "from_bytes_unchecked() must be called with a contiguous buffer"
        );
        let slice =
            unsafe { std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes()) };
        Self::from_bytes_unchecked(slice).map_err(PyErr::from)
    }

    // returns the type as well as the number of bytes read from the buffer
    #[staticmethod]
    #[pyo3(signature= (blob, trusted=false))]
    pub fn parse_rust(blob: PyBuffer<u8>, trusted: bool) -> PyResult<(Self, u32)> {
        assert!(
            blob.is_c_contiguous(),
            "parse_rust() must be called with a contiguous buffer"
        );
        let slice =
            unsafe { std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes()) };
        let mut input = Cursor::<&[u8]>::new(slice);
        if trusted {
            Self::parse::<true>(&mut input)
                .map_err(PyErr::from)
                .map(|v| (v, input.position() as u32))
        } else {
            Self::parse::<false>(&mut input)
                .map_err(PyErr::from)
                .map(|v| (v, input.position() as u32))
        }
    }

    pub fn get_hash<'p>(&self, py: Python<'p>) -> Bound<'p, PyBytes> {
        let mut ctx = Sha256::new();
        Streamable::update_digest(self, &mut ctx);
        PyBytes::new_bound(py, ctx.finalize().as_slice())
    }

    #[pyo3(name = "to_bytes")]
    pub fn py_to_bytes<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, PyBytes>> {
        let mut writer = Vec::<u8>::new();
        Streamable::stream(self, &mut writer).map_err(PyErr::from)?;
        Ok(PyBytes::new_bound(py, &writer))
    }

    pub fn stream_to_bytes<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, PyBytes>> {
        self.py_to_bytes(py)
    }

    pub fn __bytes__<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, PyBytes>> {
        self.py_to_bytes(py)
    }

    pub fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    pub fn __copy__(&self) -> Self {
        self.clone()
    }
}

impl ToJsonDict for PyWeightProof {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let wp = self.to_rust(py)?;
        let ret = PyDict::new_bound(py);
        ret.set_item("sub_epochs", wp.sub_epochs.to_json_dict(py)?)?;
        ret.set_item(
            "sub_epoch_segments",
            wp.sub_epoch_segments.to_json_dict(py)?,
        )?;
        ret.set_item("recent_chain_data", wp.recent_chain_data.to_json_dict(py)?)?;
        Ok(ret.into())
    }
}

impl FromJsonDict for PyWeightProof {
    fn from_json_dict(o: &Bound<'_, PyAny>) -> pyo3::PyResult<Self> {
        Python::with_gil(|py| {
            Ok(Self::py_new(
                py,
                FromJsonDict::from_json_dict(&o.get_item("sub_epochs")?)?,
                FromJsonDict::from_json_dict(&o.get_item("sub_epoch_segments")?)?,
                FromJsonDict::from_json_dict(&o.get_item("recent_chain_data")?)?,
            ))
        })
    }
}
