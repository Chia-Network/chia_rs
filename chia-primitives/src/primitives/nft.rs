use clvm_utils::{FromClvm, LazyNode, Result, ToClvm};
use clvmr::{allocator::NodePtr, Allocator};

use crate::singleton::SingletonStruct;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct NftState {
    pub mod_hash: [u8; 32],
    pub metadata: LazyNode,
    pub metadata_updater_puzzle_hash: [u8; 32],
    pub inner_puzzle: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct NftStateSolution {
    pub inner_solution: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct NftOwnership {
    pub mod_hash: [u8; 32],
    pub current_owner: Option<[u8; 32]>,
    pub transfer_program: LazyNode,
    pub inner_puzzle: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct NftOwnershipSolution {
    pub inner_solution: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct RoyaltyTransferProgram {
    pub singleton_struct: SingletonStruct,
    pub royalty_puzzle_hash: [u8; 32],
    pub trade_price_percentage: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NftMetadata {
    pub edition_number: u64,
    pub edition_total: u64,
    pub data_uris: Vec<String>,
    pub data_hash: Option<[u8; 32]>,
    pub metadata_uris: Vec<String>,
    pub metadata_hash: Option<[u8; 32]>,
    pub license_uris: Vec<String>,
    pub license_hash: Option<[u8; 32]>,
}

impl Default for NftMetadata {
    fn default() -> Self {
        Self {
            edition_number: 1,
            edition_total: 1,
            data_uris: Vec::new(),
            data_hash: None,
            metadata_uris: Vec::new(),
            metadata_hash: None,
            license_uris: Vec::new(),
            license_hash: None,
        }
    }
}

impl FromClvm for NftMetadata {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        let items: Vec<(String, LazyNode)> = FromClvm::from_clvm(a, node)?;
        let mut metadata = Self::default();

        for (key, LazyNode(value_ptr)) in items {
            match key.as_str() {
                "sn" => metadata.edition_number = FromClvm::from_clvm(a, value_ptr)?,
                "st" => metadata.edition_total = FromClvm::from_clvm(a, value_ptr)?,
                "u" => metadata.data_uris = FromClvm::from_clvm(a, value_ptr)?,
                "h" => metadata.data_hash = FromClvm::from_clvm(a, value_ptr)?,
                "mu" => metadata.metadata_uris = FromClvm::from_clvm(a, value_ptr)?,
                "mh" => metadata.metadata_hash = FromClvm::from_clvm(a, value_ptr)?,
                "lu" => metadata.license_uris = FromClvm::from_clvm(a, value_ptr)?,
                "lh" => metadata.license_hash = FromClvm::from_clvm(a, value_ptr)?,
                _ => (),
            }
        }

        Ok(metadata)
    }
}

impl ToClvm for NftMetadata {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let mut items: Vec<(&str, LazyNode)> = Vec::new();

        if !self.data_uris.is_empty() {
            items.push(("u", LazyNode(self.data_uris.to_clvm(a)?)));
        }

        if let Some(hash) = self.data_hash {
            items.push(("h", LazyNode(hash.to_clvm(a)?)));
        }

        if !self.metadata_uris.is_empty() {
            items.push(("mu", LazyNode(self.metadata_uris.to_clvm(a)?)));
        }

        if let Some(hash) = self.metadata_hash {
            items.push(("mh", LazyNode(hash.to_clvm(a)?)));
        }

        if !self.license_uris.is_empty() {
            items.push(("lu", LazyNode(self.license_uris.to_clvm(a)?)));
        }

        if let Some(hash) = self.license_hash {
            items.push(("lh", LazyNode(hash.to_clvm(a)?)));
        }

        items.extend(vec![
            ("sn", LazyNode(self.edition_number.to_clvm(a)?)),
            ("st", LazyNode(self.edition_total.to_clvm(a)?)),
        ]);

        items.to_clvm(a)
    }
}
