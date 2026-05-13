use crate::GeneratorInfo;
use crate::{Bytes, Bytes32, Coin, EndOfSubSlotBundle, Program, RewardChainBlock, VDFProof};
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_sha2::Sha256;
use chia_traits::Streamable;
use chia_traits::chia_error::Result;
use std::io::Cursor;

/// Full block with deferred generator parsing.
///
/// Wire format is identical to the original `FullBlock`, but the final
/// `transactions_generator` and `transactions_generator_ref_list` fields are
/// stored as a single opaque tail blob. Accessors parse that tail lazily.
#[derive(Hash, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "py-bindings", pyo3::pyclass(frozen))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FullBlock {
    pub finished_sub_slots: Vec<EndOfSubSlotBundle>,
    pub reward_chain_block: RewardChainBlock,
    pub challenge_chain_sp_proof: Option<VDFProof>,
    pub challenge_chain_ip_proof: VDFProof,
    pub reward_chain_sp_proof: Option<VDFProof>,
    pub reward_chain_ip_proof: VDFProof,
    pub infused_challenge_chain_ip_proof: Option<VDFProof>,
    pub foliage: Foliage,
    pub foliage_transaction_block: Option<FoliageTransactionBlock>,
    pub transactions_info: Option<TransactionsInfo>,

    // Combined tail (reads to EOF - everything after transactions_info)
    pub generator_info: GeneratorInfo,
}

impl FullBlock {
    #[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
    pub fn new(
        finished_sub_slots: Vec<EndOfSubSlotBundle>,
        reward_chain_block: RewardChainBlock,
        challenge_chain_sp_proof: Option<VDFProof>,
        challenge_chain_ip_proof: VDFProof,
        reward_chain_sp_proof: Option<VDFProof>,
        reward_chain_ip_proof: VDFProof,
        infused_challenge_chain_ip_proof: Option<VDFProof>,
        foliage: Foliage,
        foliage_transaction_block: Option<FoliageTransactionBlock>,
        transactions_info: Option<TransactionsInfo>,
        transactions_generator: Option<Program>,
        transactions_generator_ref_list: Vec<u32>,
    ) -> Self {
        let mut blob = Vec::new();
        transactions_generator
            .stream(&mut blob)
            .expect("streaming transactions_generator into memory cannot fail");
        transactions_generator_ref_list
            .stream(&mut blob)
            .expect("streaming transactions_generator_ref_list into memory cannot fail");

        Self {
            finished_sub_slots,
            reward_chain_block,
            challenge_chain_sp_proof,
            challenge_chain_ip_proof,
            reward_chain_sp_proof,
            reward_chain_ip_proof,
            infused_challenge_chain_ip_proof,
            foliage,
            foliage_transaction_block,
            transactions_info,
            generator_info: GeneratorInfo::from_bytes(Bytes::from(blob)),
        }
    }

    pub fn prev_header_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn header_hash(&self) -> Bytes32 {
        self.foliage.hash().into()
    }

    pub fn is_transaction_block(&self) -> bool {
        self.foliage.foliage_transaction_block_hash.is_some()
    }

    pub fn total_iters(&self) -> u128 {
        self.reward_chain_block.total_iters
    }

    pub fn height(&self) -> u32 {
        self.reward_chain_block.height
    }

    pub fn weight(&self) -> u128 {
        self.reward_chain_block.weight
    }

    pub fn get_included_reward_coins(&self) -> Vec<Coin> {
        if let Some(ti) = &self.transactions_info {
            ti.reward_claims_incorporated.clone()
        } else {
            vec![]
        }
    }

    pub fn is_fully_compactified(&self) -> bool {
        for sub_slot in &self.finished_sub_slots {
            if sub_slot.proofs.challenge_chain_slot_proof.witness_type != 0
                || !sub_slot
                    .proofs
                    .challenge_chain_slot_proof
                    .normalized_to_identity
            {
                return false;
            }
            if let Some(proof) = &sub_slot.proofs.infused_challenge_chain_slot_proof {
                if proof.witness_type != 0 || !proof.normalized_to_identity {
                    return false;
                }
            }
        }

        if let Some(proof) = &self.challenge_chain_sp_proof {
            if proof.witness_type != 0 || !proof.normalized_to_identity {
                return false;
            }
        }
        self.challenge_chain_ip_proof.witness_type == 0
            && self.challenge_chain_ip_proof.normalized_to_identity
    }

    /// Parse and return the transactions generator if present.
    pub fn transactions_generator(&self) -> Result<Option<Program>> {
        let (generator, _) = self.generator_info.parse_generator_info()?;
        Ok(generator)
    }

    /// Parse and return the transactions generator ref list.
    pub fn transactions_generator_ref_list(&self) -> Result<Vec<u32>> {
        let (_, ref_list) = self.generator_info.parse_generator_info()?;
        Ok(ref_list)
    }

    /// Parse and return both generator fields in one pass.
    pub fn parse_generator_data(&self) -> Result<(Option<Program>, Vec<u32>)> {
        self.generator_info.parse_generator_info()
    }
}

impl Streamable for FullBlock {
    fn update_digest(&self, digest: &mut Sha256) {
        self.finished_sub_slots.update_digest(digest);
        self.reward_chain_block.update_digest(digest);
        self.challenge_chain_sp_proof.update_digest(digest);
        self.challenge_chain_ip_proof.update_digest(digest);
        self.reward_chain_sp_proof.update_digest(digest);
        self.reward_chain_ip_proof.update_digest(digest);
        self.infused_challenge_chain_ip_proof.update_digest(digest);
        self.foliage.update_digest(digest);
        self.foliage_transaction_block.update_digest(digest);
        self.transactions_info.update_digest(digest);
        self.generator_info.update_digest(digest);
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.finished_sub_slots.stream(out)?;
        self.reward_chain_block.stream(out)?;
        self.challenge_chain_sp_proof.stream(out)?;
        self.challenge_chain_ip_proof.stream(out)?;
        self.reward_chain_sp_proof.stream(out)?;
        self.reward_chain_ip_proof.stream(out)?;
        self.infused_challenge_chain_ip_proof.stream(out)?;
        self.foliage.stream(out)?;
        self.foliage_transaction_block.stream(out)?;
        self.transactions_info.stream(out)?;
        self.generator_info.stream(out)?;
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        Ok(Self {
            finished_sub_slots: Vec::<EndOfSubSlotBundle>::parse::<TRUSTED>(input)?,
            reward_chain_block: RewardChainBlock::parse::<TRUSTED>(input)?,
            challenge_chain_sp_proof: Option::<VDFProof>::parse::<TRUSTED>(input)?,
            challenge_chain_ip_proof: VDFProof::parse::<TRUSTED>(input)?,
            reward_chain_sp_proof: Option::<VDFProof>::parse::<TRUSTED>(input)?,
            reward_chain_ip_proof: VDFProof::parse::<TRUSTED>(input)?,
            infused_challenge_chain_ip_proof: Option::<VDFProof>::parse::<TRUSTED>(input)?,
            foliage: Foliage::parse::<TRUSTED>(input)?,
            foliage_transaction_block: Option::<FoliageTransactionBlock>::parse::<TRUSTED>(input)?,
            transactions_info: Option::<TransactionsInfo>::parse::<TRUSTED>(input)?,
            generator_info: GeneratorInfo::parse::<TRUSTED>(input)?,
        })
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::{ChiaToPython, FromJsonDict, ToJsonDict};
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
#[pymethods]
impl FullBlock {
    #[new]
    #[pyo3(signature = (
        finished_sub_slots,
        reward_chain_block,
        challenge_chain_sp_proof,
        challenge_chain_ip_proof,
        reward_chain_sp_proof,
        reward_chain_ip_proof,
        infused_challenge_chain_ip_proof,
        foliage,
        foliage_transaction_block,
        transactions_info,
        transactions_generator,
        transactions_generator_ref_list
    ))]
    #[allow(clippy::too_many_arguments)]
    fn py_new(
        finished_sub_slots: Vec<EndOfSubSlotBundle>,
        reward_chain_block: RewardChainBlock,
        challenge_chain_sp_proof: Option<VDFProof>,
        challenge_chain_ip_proof: VDFProof,
        reward_chain_sp_proof: Option<VDFProof>,
        reward_chain_ip_proof: VDFProof,
        infused_challenge_chain_ip_proof: Option<VDFProof>,
        foliage: Foliage,
        foliage_transaction_block: Option<FoliageTransactionBlock>,
        transactions_info: Option<TransactionsInfo>,
        transactions_generator: Option<Program>,
        transactions_generator_ref_list: Vec<u32>,
    ) -> Self {
        Self::new(
            finished_sub_slots,
            reward_chain_block,
            challenge_chain_sp_proof,
            challenge_chain_ip_proof,
            reward_chain_sp_proof,
            reward_chain_ip_proof,
            infused_challenge_chain_ip_proof,
            foliage,
            foliage_transaction_block,
            transactions_info,
            transactions_generator,
            transactions_generator_ref_list,
        )
    }

    fn __repr__(&self) -> String {
        format!("{self:?}")
    }

    fn __hash__(&self) -> isize {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        Hash::hash(self, &mut hasher);
        hasher.finish() as isize
    }

    #[allow(clippy::needless_pass_by_value)]
    fn __richcmp__(
        &self,
        other: PyRef<'_, Self>,
        op: pyo3::class::basic::CompareOp,
    ) -> PyResult<Py<PyAny>> {
        use pyo3::IntoPyObjectExt;
        use pyo3::class::basic::CompareOp;
        let py = other.py();
        match op {
            CompareOp::Eq => (self == &*other).into_py_any(py),
            CompareOp::Ne => (self != &*other).into_py_any(py),
            _ => Ok(py.NotImplemented()),
        }
    }

    #[classmethod]
    #[pyo3(name = "from_bytes")]
    fn py_from_bytes(
        cls: &Bound<'_, pyo3::types::PyType>,
        py: Python<'_>,
        blob: &Bound<'_, pyo3::types::PyBytes>,
    ) -> PyResult<Py<PyAny>> {
        use pyo3::Bound;
        use pyo3::IntoPyObjectExt;
        use pyo3::prelude::PyAnyMethods;
        use pyo3::types::PyBytesMethods;
        let slice = blob.as_bytes();
        let rust_obj = Bound::new(py, Self::from_bytes(slice)?)?;

        if rust_obj.is_exact_instance(cls) {
            rust_obj.into_py_any(py)
        } else {
            let rust_py = rust_obj.into_py_any(py)?;
            let instance = cls.call_method1("from_parent", (rust_py.clone_ref(py),))?;
            Ok(instance.into_any().unbind())
        }
    }

    #[classmethod]
    #[pyo3(name = "from_bytes_unchecked")]
    fn py_from_bytes_unchecked(
        cls: &Bound<'_, pyo3::types::PyType>,
        py: Python<'_>,
        blob: &Bound<'_, pyo3::types::PyBytes>,
    ) -> PyResult<Py<PyAny>> {
        use pyo3::Bound;
        use pyo3::IntoPyObjectExt;
        use pyo3::prelude::PyAnyMethods;
        use pyo3::types::PyBytesMethods;
        let slice = blob.as_bytes();
        let rust_obj = Bound::new(py, Self::from_bytes_unchecked(slice)?)?;

        if rust_obj.is_exact_instance(cls) {
            rust_obj.into_py_any(py)
        } else {
            let rust_py = rust_obj.into_py_any(py)?;
            let instance = cls.call_method1("from_parent", (rust_py.clone_ref(py),))?;
            Ok(instance.into_any().unbind())
        }
    }

    #[classmethod]
    #[pyo3(signature = (blob, trusted=false))]
    fn parse_rust(
        cls: &Bound<'_, pyo3::types::PyType>,
        py: Python<'_>,
        blob: &Bound<'_, pyo3::types::PyBytes>,
        trusted: bool,
    ) -> PyResult<(Py<PyAny>, u32)> {
        use pyo3::Bound;
        use pyo3::IntoPyObjectExt;
        use pyo3::prelude::PyAnyMethods;
        use pyo3::types::PyBytesMethods;
        let slice = blob.as_bytes();
        let mut input = Cursor::<&[u8]>::new(slice);
        let rust_obj = if trusted {
            Self::parse::<true>(&mut input)
        } else {
            Self::parse::<false>(&mut input)
        }?;
        let position = input.position() as u32;
        let rust_bound = Bound::new(py, rust_obj)?;

        if rust_bound.is_exact_instance(cls) {
            Ok((rust_bound.into_py_any(py)?, position))
        } else {
            let rust_py = rust_bound.into_py_any(py)?;
            let instance = cls.call_method1("from_parent", (rust_py.clone_ref(py),))?;
            Ok((instance.into_any().unbind(), position))
        }
    }

    #[getter]
    fn finished_sub_slots<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.finished_sub_slots.to_python(py)
    }

    #[getter]
    fn reward_chain_block<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.reward_chain_block.to_python(py)
    }

    #[getter]
    fn challenge_chain_sp_proof<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.challenge_chain_sp_proof.to_python(py)
    }

    #[getter]
    fn challenge_chain_ip_proof<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.challenge_chain_ip_proof.to_python(py)
    }

    #[getter]
    fn reward_chain_sp_proof<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.reward_chain_sp_proof.to_python(py)
    }

    #[getter]
    fn reward_chain_ip_proof<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.reward_chain_ip_proof.to_python(py)
    }

    #[getter]
    fn infused_challenge_chain_ip_proof<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.infused_challenge_chain_ip_proof.to_python(py)
    }

    #[getter]
    fn foliage<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.foliage.to_python(py)
    }

    #[getter]
    fn foliage_transaction_block<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.foliage_transaction_block.to_python(py)
    }

    #[getter]
    fn transactions_info<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        self.transactions_info.to_python(py)
    }

    #[getter]
    #[pyo3(name = "prev_header_hash")]
    fn py_prev_header_hash(&self) -> Bytes32 {
        self.prev_header_hash()
    }

    #[getter]
    #[pyo3(name = "header_hash")]
    fn py_header_hash(&self) -> Bytes32 {
        self.header_hash()
    }

    #[pyo3(name = "is_transaction_block")]
    fn py_is_transaction_block(&self) -> bool {
        self.is_transaction_block()
    }

    #[getter]
    #[pyo3(name = "total_iters")]
    fn py_total_iters<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.total_iters(), py)
    }

    #[getter]
    #[pyo3(name = "height")]
    fn py_height<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.height(), py)
    }

    #[getter]
    #[pyo3(name = "weight")]
    fn py_weight<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.weight(), py)
    }

    #[pyo3(name = "get_included_reward_coins")]
    fn py_get_included_reward_coins(&self) -> Vec<Coin> {
        self.get_included_reward_coins()
    }

    #[pyo3(name = "is_fully_compactified")]
    fn py_is_fully_compactified(&self) -> bool {
        self.is_fully_compactified()
    }

    #[getter]
    #[pyo3(name = "transactions_generator")]
    fn py_transactions_generator(&self) -> PyResult<Option<Program>> {
        self.transactions_generator()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    #[getter]
    #[pyo3(name = "transactions_generator_ref_list")]
    fn py_transactions_generator_ref_list(&self) -> PyResult<Vec<u32>> {
        self.transactions_generator_ref_list()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    #[pyo3(signature = (**kwargs))]
    fn replace(&self, kwargs: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let mut finished_sub_slots = self.finished_sub_slots.clone();
        let mut reward_chain_block = self.reward_chain_block.clone();
        let mut challenge_chain_sp_proof = self.challenge_chain_sp_proof.clone();
        let mut challenge_chain_ip_proof = self.challenge_chain_ip_proof.clone();
        let mut reward_chain_sp_proof = self.reward_chain_sp_proof.clone();
        let mut reward_chain_ip_proof = self.reward_chain_ip_proof.clone();
        let mut infused_challenge_chain_ip_proof = self.infused_challenge_chain_ip_proof.clone();
        let mut foliage = self.foliage.clone();
        let mut foliage_transaction_block = self.foliage_transaction_block.clone();
        let mut transactions_info = self.transactions_info.clone();
        let (mut transactions_generator, mut transactions_generator_ref_list) = self
            .parse_generator_data()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        if let Some(kwargs) = kwargs {
            use pyo3::prelude::{PyAnyMethods, PyDictMethods};
            for (field, value) in kwargs.iter() {
                let field = field.extract::<String>()?;
                match field.as_str() {
                    "finished_sub_slots" => finished_sub_slots = value.extract()?,
                    "reward_chain_block" => reward_chain_block = value.extract()?,
                    "challenge_chain_sp_proof" => challenge_chain_sp_proof = value.extract()?,
                    "challenge_chain_ip_proof" => challenge_chain_ip_proof = value.extract()?,
                    "reward_chain_sp_proof" => reward_chain_sp_proof = value.extract()?,
                    "reward_chain_ip_proof" => reward_chain_ip_proof = value.extract()?,
                    "infused_challenge_chain_ip_proof" => {
                        infused_challenge_chain_ip_proof = value.extract()?;
                    }
                    "foliage" => foliage = value.extract()?,
                    "foliage_transaction_block" => foliage_transaction_block = value.extract()?,
                    "transactions_info" => transactions_info = value.extract()?,
                    "transactions_generator" => transactions_generator = value.extract()?,
                    "transactions_generator_ref_list" => {
                        transactions_generator_ref_list = value.extract()?;
                    }
                    _ => {
                        return Err(pyo3::exceptions::PyKeyError::new_err(format!(
                            "unknown field {field}"
                        )));
                    }
                }
            }
        }

        Ok(Self::new(
            finished_sub_slots,
            reward_chain_block,
            challenge_chain_sp_proof,
            challenge_chain_ip_proof,
            reward_chain_sp_proof,
            reward_chain_ip_proof,
            infused_challenge_chain_ip_proof,
            foliage,
            foliage_transaction_block,
            transactions_info,
            transactions_generator,
            transactions_generator_ref_list,
        ))
    }

    fn get_hash<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        use pyo3::IntoPyObjectExt;
        use pyo3::prelude::PyAnyMethods;
        use pyo3::types::PyModule;
        let mut ctx = Sha256::new();
        Streamable::update_digest(self, &mut ctx);
        let bytes_module = PyModule::import(py, "chia_rs.sized_bytes")?;
        let ty = bytes_module.getattr("bytes32")?;
        let digest = ctx.finalize().into_py_any(py)?;
        ty.call1((digest,))
    }

    #[pyo3(name = "to_bytes")]
    fn py_to_bytes<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, pyo3::types::PyBytes>> {
        let mut writer = Vec::<u8>::new();
        Streamable::stream(self, &mut writer)?;
        Ok(pyo3::types::PyBytes::new(py, &writer))
    }

    fn stream_to_bytes<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, pyo3::types::PyBytes>> {
        self.py_to_bytes(py)
    }

    fn __bytes__<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, pyo3::types::PyBytes>> {
        self.py_to_bytes(py)
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn to_json_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        ToJsonDict::to_json_dict(self, py)
    }

    #[classmethod]
    #[pyo3(signature=(json_dict))]
    fn from_json_dict(
        cls: &Bound<'_, pyo3::types::PyType>,
        py: Python<'_>,
        json_dict: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        use pyo3::Bound;
        use pyo3::IntoPyObjectExt;
        use pyo3::prelude::PyAnyMethods;
        let rust_obj = Bound::new(py, <Self as FromJsonDict>::from_json_dict(json_dict)?)?;

        if rust_obj.is_exact_instance(cls) {
            rust_obj.into_py_any(py)
        } else {
            let rust_py = rust_obj.into_py_any(py)?;
            let instance = cls.call_method1("from_parent", (rust_py.clone_ref(py),))?;
            Ok(instance.into_any().unbind())
        }
    }
}

#[cfg(feature = "py-bindings")]
impl ChiaToPython for FullBlock {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        Ok(Py::new(py, self.clone())?.into_bound(py).into_any())
    }
}

#[cfg(feature = "py-bindings")]
impl ToJsonDict for FullBlock {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        use pyo3::prelude::PyDictMethods;
        let ret = pyo3::types::PyDict::new(py);
        let (transactions_generator, transactions_generator_ref_list) = self
            .parse_generator_data()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        ret.set_item(
            "finished_sub_slots",
            self.finished_sub_slots.to_json_dict(py)?,
        )?;
        ret.set_item(
            "reward_chain_block",
            self.reward_chain_block.to_json_dict(py)?,
        )?;
        ret.set_item(
            "challenge_chain_sp_proof",
            self.challenge_chain_sp_proof.to_json_dict(py)?,
        )?;
        ret.set_item(
            "challenge_chain_ip_proof",
            self.challenge_chain_ip_proof.to_json_dict(py)?,
        )?;
        ret.set_item(
            "reward_chain_sp_proof",
            self.reward_chain_sp_proof.to_json_dict(py)?,
        )?;
        ret.set_item(
            "reward_chain_ip_proof",
            self.reward_chain_ip_proof.to_json_dict(py)?,
        )?;
        ret.set_item(
            "infused_challenge_chain_ip_proof",
            self.infused_challenge_chain_ip_proof.to_json_dict(py)?,
        )?;
        ret.set_item("foliage", self.foliage.to_json_dict(py)?)?;
        ret.set_item(
            "foliage_transaction_block",
            self.foliage_transaction_block.to_json_dict(py)?,
        )?;
        ret.set_item(
            "transactions_info",
            self.transactions_info.to_json_dict(py)?,
        )?;
        ret.set_item(
            "transactions_generator",
            transactions_generator.to_json_dict(py)?,
        )?;
        ret.set_item(
            "transactions_generator_ref_list",
            transactions_generator_ref_list.to_json_dict(py)?,
        )?;
        Ok(ret.into_any().unbind())
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for FullBlock {
    fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
        use pyo3::prelude::PyAnyMethods;
        Ok(Self::new(
            <Vec<EndOfSubSlotBundle> as FromJsonDict>::from_json_dict(
                &o.get_item("finished_sub_slots")?,
            )?,
            <RewardChainBlock as FromJsonDict>::from_json_dict(&o.get_item("reward_chain_block")?)?,
            <Option<VDFProof> as FromJsonDict>::from_json_dict(
                &o.get_item("challenge_chain_sp_proof")?,
            )?,
            <VDFProof as FromJsonDict>::from_json_dict(&o.get_item("challenge_chain_ip_proof")?)?,
            <Option<VDFProof> as FromJsonDict>::from_json_dict(
                &o.get_item("reward_chain_sp_proof")?,
            )?,
            <VDFProof as FromJsonDict>::from_json_dict(&o.get_item("reward_chain_ip_proof")?)?,
            <Option<VDFProof> as FromJsonDict>::from_json_dict(
                &o.get_item("infused_challenge_chain_ip_proof")?,
            )?,
            <Foliage as FromJsonDict>::from_json_dict(&o.get_item("foliage")?)?,
            <Option<FoliageTransactionBlock> as FromJsonDict>::from_json_dict(
                &o.get_item("foliage_transaction_block")?,
            )?,
            <Option<TransactionsInfo> as FromJsonDict>::from_json_dict(
                &o.get_item("transactions_info")?,
            )?,
            <Option<Program> as FromJsonDict>::from_json_dict(
                &o.get_item("transactions_generator")?,
            )?,
            <Vec<u32> as FromJsonDict>::from_json_dict(
                &o.get_item("transactions_generator_ref_list")?,
            )?,
        ))
    }
}
