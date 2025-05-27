use chia_protocol::Bytes32;
use chia_puzzles::{
    NFT_INTERMEDIATE_LAUNCHER_HASH, NFT_METADATA_UPDATER_DEFAULT_HASH, NFT_OWNERSHIP_LAYER_HASH,
    NFT_OWNERSHIP_TRANSFER_PROGRAM_ONE_WAY_CLAIM_WITH_ROYALTIES_HASH, NFT_STATE_LAYER_HASH,
    SINGLETON_LAUNCHER_HASH,
};
use clvm_traits::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, Raw, ToClvm, ToClvmError};
use clvm_utils::{CurriedProgram, ToTreeHash, TreeHash};

use crate::singleton::SingletonStruct;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct NftIntermediateLauncherArgs {
    pub launcher_puzzle_hash: Bytes32,
    pub mint_number: usize,
    pub mint_total: usize,
}

impl NftIntermediateLauncherArgs {
    pub fn new(mint_number: usize, mint_total: usize) -> Self {
        Self {
            launcher_puzzle_hash: SINGLETON_LAUNCHER_HASH.into(),
            mint_number,
            mint_total,
        }
    }

    pub fn curry_tree_hash(mint_number: usize, mint_total: usize) -> TreeHash {
        CurriedProgram {
            program: TreeHash::new(NFT_INTERMEDIATE_LAUNCHER_HASH),
            args: NftIntermediateLauncherArgs {
                launcher_puzzle_hash: SINGLETON_LAUNCHER_HASH.into(),
                mint_number,
                mint_total,
            },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct NftStateLayerArgs<I, M> {
    pub mod_hash: Bytes32,
    pub metadata: M,
    pub metadata_updater_puzzle_hash: Bytes32,
    pub inner_puzzle: I,
}

impl<I, M> NftStateLayerArgs<I, M> {
    pub fn new(metadata: M, inner_puzzle: I) -> Self {
        Self {
            mod_hash: NFT_STATE_LAYER_HASH.into(),
            metadata,
            metadata_updater_puzzle_hash: NFT_METADATA_UPDATER_DEFAULT_HASH.into(),
            inner_puzzle,
        }
    }
}

impl NftStateLayerArgs<TreeHash, TreeHash> {
    pub fn curry_tree_hash(metadata: TreeHash, inner_puzzle: TreeHash) -> TreeHash {
        CurriedProgram {
            program: TreeHash::new(NFT_STATE_LAYER_HASH),
            args: NftStateLayerArgs {
                mod_hash: NFT_STATE_LAYER_HASH.into(),
                metadata,
                metadata_updater_puzzle_hash: NFT_METADATA_UPDATER_DEFAULT_HASH.into(),
                inner_puzzle,
            },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct NftStateLayerSolution<I> {
    pub inner_solution: I,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct NftOwnershipLayerArgs<I, P> {
    pub mod_hash: Bytes32,
    pub current_owner: Option<Bytes32>,
    pub transfer_program: P,
    pub inner_puzzle: I,
}

impl<I, P> NftOwnershipLayerArgs<I, P> {
    pub fn new(current_owner: Option<Bytes32>, transfer_program: P, inner_puzzle: I) -> Self {
        Self {
            mod_hash: NFT_OWNERSHIP_LAYER_HASH.into(),
            current_owner,
            transfer_program,
            inner_puzzle,
        }
    }
}

impl NftOwnershipLayerArgs<TreeHash, TreeHash> {
    pub fn curry_tree_hash(
        current_owner: Option<Bytes32>,
        transfer_program: TreeHash,
        inner_puzzle: TreeHash,
    ) -> TreeHash {
        CurriedProgram {
            program: TreeHash::new(NFT_OWNERSHIP_LAYER_HASH),
            args: NftOwnershipLayerArgs {
                mod_hash: NFT_OWNERSHIP_LAYER_HASH.into(),
                current_owner,
                transfer_program,
                inner_puzzle,
            },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct NftOwnershipLayerSolution<I> {
    pub inner_solution: I,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct NftRoyaltyTransferPuzzleArgs {
    pub singleton_struct: SingletonStruct,
    pub royalty_puzzle_hash: Bytes32,
    /// The royalty percentage expressed as ten-thousandths.
    /// For example, 300 represents 3%.
    pub royalty_ten_thousandths: u16,
}

impl NftRoyaltyTransferPuzzleArgs {
    pub fn new(
        launcher_id: Bytes32,
        royalty_puzzle_hash: Bytes32,
        royalty_ten_thousandths: u16,
    ) -> Self {
        Self {
            singleton_struct: SingletonStruct::new(launcher_id),
            royalty_puzzle_hash,
            royalty_ten_thousandths,
        }
    }

    pub fn curry_tree_hash(
        launcher_id: Bytes32,
        royalty_puzzle_hash: Bytes32,
        royalty_ten_thousandths: u16,
    ) -> TreeHash {
        CurriedProgram {
            program: TreeHash::new(
                NFT_OWNERSHIP_TRANSFER_PROGRAM_ONE_WAY_CLAIM_WITH_ROYALTIES_HASH,
            ),
            args: NftRoyaltyTransferPuzzleArgs {
                singleton_struct: SingletonStruct::new(launcher_id),
                royalty_puzzle_hash,
                royalty_ten_thousandths,
            },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct NftMetadata {
    pub edition_number: u64,
    pub edition_total: u64,
    pub data_uris: Vec<String>,
    pub data_hash: Option<Bytes32>,
    pub metadata_uris: Vec<String>,
    pub metadata_hash: Option<Bytes32>,
    pub license_uris: Vec<String>,
    pub license_hash: Option<Bytes32>,
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

impl<N, D: ClvmDecoder<Node = N>> FromClvm<D> for NftMetadata {
    fn from_clvm(decoder: &D, node: N) -> Result<Self, FromClvmError> {
        let items: Vec<(String, Raw<N>)> = FromClvm::from_clvm(decoder, node)?;
        let mut metadata = Self::default();

        for (key, value_ptr) in items {
            match key.as_str() {
                "sn" => metadata.edition_number = FromClvm::from_clvm(decoder, value_ptr.0)?,
                "st" => metadata.edition_total = FromClvm::from_clvm(decoder, value_ptr.0)?,
                "u" => metadata.data_uris = FromClvm::from_clvm(decoder, value_ptr.0)?,
                "h" => metadata.data_hash = FromClvm::from_clvm(decoder, value_ptr.0)?,
                "mu" => metadata.metadata_uris = FromClvm::from_clvm(decoder, value_ptr.0)?,
                "mh" => metadata.metadata_hash = FromClvm::from_clvm(decoder, value_ptr.0)?,
                "lu" => metadata.license_uris = FromClvm::from_clvm(decoder, value_ptr.0)?,
                "lh" => metadata.license_hash = FromClvm::from_clvm(decoder, value_ptr.0)?,
                _ => (),
            }
        }

        Ok(metadata)
    }
}

impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for NftMetadata {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        let mut items: Vec<(&str, Raw<N>)> = Vec::new();

        if !self.data_uris.is_empty() {
            items.push(("u", Raw(self.data_uris.to_clvm(encoder)?)));
        }

        if let Some(hash) = self.data_hash {
            items.push(("h", Raw(hash.to_clvm(encoder)?)));
        }

        if !self.metadata_uris.is_empty() {
            items.push(("mu", Raw(self.metadata_uris.to_clvm(encoder)?)));
        }

        if let Some(hash) = self.metadata_hash {
            items.push(("mh", Raw(hash.to_clvm(encoder)?)));
        }

        if !self.license_uris.is_empty() {
            items.push(("lu", Raw(self.license_uris.to_clvm(encoder)?)));
        }

        if let Some(hash) = self.license_hash {
            items.push(("lh", Raw(hash.to_clvm(encoder)?)));
        }

        items.extend(vec![
            ("sn", Raw(self.edition_number.to_clvm(encoder)?)),
            ("st", Raw(self.edition_total.to_clvm(encoder)?)),
        ]);

        items.to_clvm(encoder)
    }
}
