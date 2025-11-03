use chia_streamable_macro::streamable;

use crate::{Bytes32, ProofOfSpace, TwoOption, VDFInfo};
use chia_bls::G2Element;

#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};

#[cfg(feature = "py-bindings")]
use pyo3::pymethods;

#[streamable]
pub struct RewardChainBlockUnfinished {
    total_iters: u128,
    signage_point_index: u8,
    pos_ss_cc_challenge_hash: Bytes32,
    proof_of_space: ProofOfSpace,
    challenge_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    challenge_chain_sp_signature: G2Element,
    reward_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    reward_chain_sp_signature: G2Element,
}

#[streamable(no_json)]
pub struct RewardChainBlock {
    weight: u128,
    height: u32,
    total_iters: u128,
    signage_point_index: u8,
    pos_ss_cc_challenge_hash: Bytes32,
    proof_of_space: ProofOfSpace,
    challenge_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    challenge_chain_sp_signature: G2Element,
    challenge_chain_ip_vdf: VDFInfo,
    reward_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    reward_chain_sp_signature: G2Element,
    reward_chain_ip_vdf: VDFInfo,
    // VDFInfo is set Iff deficit < 16
    infused_challenge_chain_ip_vdf_and_merkle_root: TwoOption<VDFInfo, Bytes32>,
    is_transaction_block: bool,
}

#[cfg_attr(feature = "py-bindings", pymethods)]
impl RewardChainBlock {
    pub fn get_unfinished(&self) -> RewardChainBlockUnfinished {
        RewardChainBlockUnfinished {
            total_iters: self.total_iters,
            signage_point_index: self.signage_point_index,
            pos_ss_cc_challenge_hash: self.pos_ss_cc_challenge_hash,
            proof_of_space: self.proof_of_space.clone(),
            challenge_chain_sp_vdf: self.challenge_chain_sp_vdf.clone(),
            challenge_chain_sp_signature: self.challenge_chain_sp_signature.clone(),
            reward_chain_sp_vdf: self.reward_chain_sp_vdf.clone(),
            reward_chain_sp_signature: self.reward_chain_sp_signature.clone(),
        }
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl RewardChainBlock {
    #[getter]
    pub fn infused_challenge_chain_ip_vdf(&self) -> Option<VDFInfo> {
        self.infused_challenge_chain_ip_vdf_and_merkle_root
            .0
            .clone()
    }

    #[getter]
    pub fn merkle_root(&self) -> Option<Bytes32> {
        self.infused_challenge_chain_ip_vdf_and_merkle_root.1
    }
}

#[cfg(feature = "py-bindings")]
impl ToJsonDict for RewardChainBlock {
    fn to_json_dict(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::PyObject> {
        use pyo3::prelude::PyDictMethods;
        let ret = pyo3::types::PyDict::new(py);

        ret.set_item("weight", self.weight.to_json_dict(py)?)?;
        ret.set_item("height", self.height.to_json_dict(py)?)?;
        ret.set_item("total_iters", self.total_iters.to_json_dict(py)?)?;
        ret.set_item(
            "signage_point_index",
            self.signage_point_index.to_json_dict(py)?,
        )?;
        ret.set_item(
            "pos_ss_cc_challenge_hash",
            self.pos_ss_cc_challenge_hash.to_json_dict(py)?,
        )?;
        ret.set_item("proof_of_space", self.proof_of_space.to_json_dict(py)?)?;
        ret.set_item(
            "challenge_chain_sp_vdf",
            self.challenge_chain_sp_vdf.to_json_dict(py)?,
        )?;
        ret.set_item(
            "challenge_chain_sp_signature",
            self.challenge_chain_sp_signature.to_json_dict(py)?,
        )?;
        ret.set_item(
            "challenge_chain_ip_vdf",
            self.challenge_chain_ip_vdf.to_json_dict(py)?,
        )?;
        ret.set_item(
            "reward_chain_sp_vdf",
            self.reward_chain_sp_vdf.to_json_dict(py)?,
        )?;
        ret.set_item(
            "reward_chain_sp_signature",
            self.reward_chain_sp_signature.to_json_dict(py)?,
        )?;
        ret.set_item(
            "reward_chain_ip_vdf",
            self.reward_chain_ip_vdf.to_json_dict(py)?,
        )?;
        ret.set_item(
            "infused_challenge_chain_ip_vdf",
            self.infused_challenge_chain_ip_vdf_and_merkle_root
                .0
                .to_json_dict(py)?,
        )?;
        ret.set_item(
            "merkle_root",
            self.infused_challenge_chain_ip_vdf_and_merkle_root
                .1
                .to_json_dict(py)?,
        )?;
        ret.set_item(
            "is_transaction_block",
            self.is_transaction_block.to_json_dict(py)?,
        )?;
        Ok(ret.into())
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for RewardChainBlock {
    fn from_json_dict(o: &pyo3::Bound<'_, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        use pyo3::prelude::PyAnyMethods;
        Ok(Self {
            weight: u128::from_json_dict(&o.get_item("weight")?)?,
            height: u32::from_json_dict(&o.get_item("height")?)?,
            total_iters: u128::from_json_dict(&o.get_item("total_iters")?)?,
            signage_point_index: u8::from_json_dict(&o.get_item("signage_point_index")?)?,
            pos_ss_cc_challenge_hash: Bytes32::from_json_dict(
                &o.get_item("pos_ss_cc_challenge_hash")?,
            )?,
            proof_of_space: <ProofOfSpace as FromJsonDict>::from_json_dict(
                &o.get_item("proof_of_space")?,
            )?,
            challenge_chain_sp_vdf: Option::<VDFInfo>::from_json_dict(
                &o.get_item("challenge_change_sp_vdf")?,
            )?,
            challenge_chain_sp_signature: <G2Element as FromJsonDict>::from_json_dict(
                &o.get_item("challenge_chain_sp_signature")?,
            )?,
            challenge_chain_ip_vdf: <VDFInfo as FromJsonDict>::from_json_dict(
                &o.get_item("challenge_chain_ip_vdf")?,
            )?,
            reward_chain_sp_vdf: <Option<VDFInfo> as FromJsonDict>::from_json_dict(
                &o.get_item("reward_chain_sp_vdf")?,
            )?,
            reward_chain_sp_signature: <G2Element as FromJsonDict>::from_json_dict(
                &o.get_item("reward_chain_sp_signature")?,
            )?,
            reward_chain_ip_vdf: <VDFInfo as FromJsonDict>::from_json_dict(
                &o.get_item("reward_chain_ip_vdf")?,
            )?,
            infused_challenge_chain_ip_vdf_and_merkle_root: TwoOption(
                <Option<VDFInfo> as FromJsonDict>::from_json_dict(
                    &o.get_item("infused_challenge_chain_ip_vdf")?,
                )?,
                <Option<Bytes32> as FromJsonDict>::from_json_dict(&o.get_item("merkle_root")?)?,
            ),
            is_transaction_block: <bool as FromJsonDict>::from_json_dict(
                &o.get_item("is_transaction_block")?,
            )?,
        })
    }
}
