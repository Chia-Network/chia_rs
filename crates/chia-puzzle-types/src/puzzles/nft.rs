use chia_protocol::Bytes32;
use clvm_traits::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, Raw, ToClvm, ToClvmError};
use clvm_utils::{CurriedProgram, ToTreeHash, TreeHash};
use hex_literal::hex;

use crate::singleton::{SingletonStruct, SINGLETON_LAUNCHER_PUZZLE_HASH};

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
            launcher_puzzle_hash: SINGLETON_LAUNCHER_PUZZLE_HASH.into(),
            mint_number,
            mint_total,
        }
    }

    pub fn curry_tree_hash(mint_number: usize, mint_total: usize) -> TreeHash {
        CurriedProgram {
            program: NFT_INTERMEDIATE_LAUNCHER_PUZZLE_HASH,
            args: NftIntermediateLauncherArgs {
                launcher_puzzle_hash: SINGLETON_LAUNCHER_PUZZLE_HASH.into(),
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
            mod_hash: NFT_STATE_LAYER_PUZZLE_HASH.into(),
            metadata,
            metadata_updater_puzzle_hash: NFT_METADATA_UPDATER_PUZZLE_HASH.into(),
            inner_puzzle,
        }
    }
}

impl NftStateLayerArgs<TreeHash, TreeHash> {
    pub fn curry_tree_hash(metadata: TreeHash, inner_puzzle: TreeHash) -> TreeHash {
        CurriedProgram {
            program: NFT_STATE_LAYER_PUZZLE_HASH,
            args: NftStateLayerArgs {
                mod_hash: NFT_STATE_LAYER_PUZZLE_HASH.into(),
                metadata,
                metadata_updater_puzzle_hash: NFT_METADATA_UPDATER_PUZZLE_HASH.into(),
                inner_puzzle,
            },
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(solution)]
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
            mod_hash: NFT_OWNERSHIP_LAYER_PUZZLE_HASH.into(),
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
            program: NFT_OWNERSHIP_LAYER_PUZZLE_HASH,
            args: NftOwnershipLayerArgs {
                mod_hash: NFT_OWNERSHIP_LAYER_PUZZLE_HASH.into(),
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
#[clvm(solution)]
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
            program: NFT_ROYALTY_TRANSFER_PUZZLE_HASH,
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

/// This is the puzzle reveal of the [NFT1 state layer](https://chialisp.com/nfts) puzzle.
pub const NFT_STATE_LAYER_PUZZLE: [u8; 827] = hex!(
    "
    ff02ffff01ff02ff3effff04ff02ffff04ff05ffff04ffff02ff2fff5f80ffff
    04ff80ffff04ffff04ffff04ff0bffff04ff17ff808080ffff01ff808080ffff
    01ff8080808080808080ffff04ffff01ffffff0233ff04ff0101ffff02ff02ff
    ff03ff05ffff01ff02ff1affff04ff02ffff04ff0dffff04ffff0bff12ffff0b
    ff2cff1480ffff0bff12ffff0bff12ffff0bff2cff3c80ff0980ffff0bff12ff
    0bffff0bff2cff8080808080ff8080808080ffff010b80ff0180ffff0bff12ff
    ff0bff2cff1080ffff0bff12ffff0bff12ffff0bff2cff3c80ff0580ffff0bff
    12ffff02ff1affff04ff02ffff04ff07ffff04ffff0bff2cff2c80ff80808080
    80ffff0bff2cff8080808080ffff02ffff03ffff07ff0580ffff01ff0bffff01
    02ffff02ff2effff04ff02ffff04ff09ff80808080ffff02ff2effff04ff02ff
    ff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff02ffff03ff
    0bffff01ff02ffff03ffff09ff23ff1880ffff01ff02ffff03ffff18ff81b3ff
    2c80ffff01ff02ffff03ffff20ff1780ffff01ff02ff3effff04ff02ffff04ff
    05ffff04ff1bffff04ff33ffff04ff2fffff04ff5fff8080808080808080ffff
    01ff088080ff0180ffff01ff04ff13ffff02ff3effff04ff02ffff04ff05ffff
    04ff1bffff04ff17ffff04ff2fffff04ff5fff80808080808080808080ff0180
    ffff01ff02ffff03ffff09ff23ffff0181e880ffff01ff02ff3effff04ff02ff
    ff04ff05ffff04ff1bffff04ff17ffff04ffff02ffff03ffff22ffff09ffff02
    ff2effff04ff02ffff04ff53ff80808080ff82014f80ffff20ff5f8080ffff01
    ff02ff53ffff04ff818fffff04ff82014fffff04ff81b3ff8080808080ffff01
    ff088080ff0180ffff04ff2cff8080808080808080ffff01ff04ff13ffff02ff
    3effff04ff02ffff04ff05ffff04ff1bffff04ff17ffff04ff2fffff04ff5fff
    80808080808080808080ff018080ff0180ffff01ff04ffff04ff18ffff04ffff
    02ff16ffff04ff02ffff04ff05ffff04ff27ffff04ffff0bff2cff82014f80ff
    ff04ffff02ff2effff04ff02ffff04ff818fff80808080ffff04ffff0bff2cff
    0580ff8080808080808080ff378080ff81af8080ff0180ff018080
    "
);

/// This is the puzzle hash of the [NFT1 state layer](https://chialisp.com/nfts) puzzle.
pub const NFT_STATE_LAYER_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    a04d9f57764f54a43e4030befb4d80026e870519aaa66334aef8304f5d0393c2
    "
));

/// This is the puzzle reveal of the [NFT1 ownership layer](https://chialisp.com/nfts) puzzle.
pub const NFT_OWNERSHIP_LAYER_PUZZLE: [u8; 1226] = hex!(
    "
    ff02ffff01ff02ff26ffff04ff02ffff04ff05ffff04ff17ffff04ff0bffff04
    ffff02ff2fff5f80ff80808080808080ffff04ffff01ffffff82ad4cff0233ff
    ff3e04ff81f601ffffff0102ffff02ffff03ff05ffff01ff02ff2affff04ff02
    ffff04ff0dffff04ffff0bff32ffff0bff3cff3480ffff0bff32ffff0bff32ff
    ff0bff3cff2280ff0980ffff0bff32ff0bffff0bff3cff8080808080ff808080
    8080ffff010b80ff0180ff04ffff04ff38ffff04ffff02ff36ffff04ff02ffff
    04ff05ffff04ff27ffff04ffff02ff2effff04ff02ffff04ffff02ffff03ff81
    afffff0181afffff010b80ff0180ff80808080ffff04ffff0bff3cff4f80ffff
    04ffff0bff3cff0580ff8080808080808080ff378080ff82016f80ffffff02ff
    3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff2fff
    ff01ff80ff808080808080808080ff0bff32ffff0bff3cff2880ffff0bff32ff
    ff0bff32ffff0bff3cff2280ff0580ffff0bff32ffff02ff2affff04ff02ffff
    04ff07ffff04ffff0bff3cff3c80ff8080808080ffff0bff3cff8080808080ff
    ff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff2effff04ff02ffff
    04ff09ff80808080ffff02ff2effff04ff02ffff04ff0dff8080808080ffff01
    ff0bffff0101ff058080ff0180ff02ffff03ff5fffff01ff02ffff03ffff09ff
    82011fff3880ffff01ff02ffff03ffff09ffff18ff82059f80ff3c80ffff01ff
    02ffff03ffff20ff81bf80ffff01ff02ff3effff04ff02ffff04ff05ffff04ff
    0bffff04ff17ffff04ff2fffff04ff81dfffff04ff82019fffff04ff82017fff
    80808080808080808080ffff01ff088080ff0180ffff01ff04ff819fffff02ff
    3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81df
    ffff04ff81bfffff04ff82017fff808080808080808080808080ff0180ffff01
    ff02ffff03ffff09ff82011fff2c80ffff01ff02ffff03ffff20ff82017f80ff
    ff01ff04ffff04ff24ffff04ffff0eff10ffff02ff2effff04ff02ffff04ff82
    019fff8080808080ff808080ffff02ff3effff04ff02ffff04ff05ffff04ff0b
    ffff04ff17ffff04ff2fffff04ff81dfffff04ff81bfffff04ffff02ff0bffff
    04ff17ffff04ff2fffff04ff82019fff8080808080ff80808080808080808080
    80ffff01ff088080ff0180ffff01ff02ffff03ffff09ff82011fff2480ffff01
    ff02ffff03ffff20ffff02ffff03ffff09ffff0122ffff0dff82029f8080ffff
    01ff02ffff03ffff09ffff0cff82029fff80ffff010280ff1080ffff01ff0101
    ff8080ff0180ff8080ff018080ffff01ff04ff819fffff02ff3effff04ff02ff
    ff04ff05ffff04ff0bffff04ff17ffff04ff2fffff04ff81dfffff04ff81bfff
    ff04ff82017fff8080808080808080808080ffff01ff088080ff0180ffff01ff
    04ff819fffff02ff3effff04ff02ffff04ff05ffff04ff0bffff04ff17ffff04
    ff2fffff04ff81dfffff04ff81bfffff04ff82017fff80808080808080808080
    8080ff018080ff018080ff0180ffff01ff02ff3affff04ff02ffff04ff05ffff
    04ff0bffff04ff81bfffff04ffff02ffff03ff82017fffff0182017fffff01ff
    02ff0bffff04ff17ffff04ff2fffff01ff808080808080ff0180ff8080808080
    808080ff0180ff018080
    "
);

/// This is the puzzle hash of the [NFT1 ownership layer](https://chialisp.com/nfts) puzzle.
pub const NFT_OWNERSHIP_LAYER_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    c5abea79afaa001b5427dfa0c8cf42ca6f38f5841b78f9b3c252733eb2de2726
    "
));

/// This is the puzzle reveal of the [NFT1 royalty transfer](https://chialisp.com/nfts) puzzle.
pub const NFT_ROYALTY_TRANSFER_PUZZLE: [u8; 687] = hex!(
    "
    ff02ffff01ff02ffff03ff81bfffff01ff04ff82013fffff04ff80ffff04ffff
    02ffff03ffff22ff82013fffff20ffff09ff82013fff2f808080ffff01ff04ff
    ff04ff10ffff04ffff0bffff02ff2effff04ff02ffff04ff09ffff04ff8205bf
    ffff04ffff02ff3effff04ff02ffff04ffff04ff09ffff04ff82013fff1d8080
    ff80808080ff808080808080ff1580ff808080ffff02ff16ffff04ff02ffff04
    ff0bffff04ff17ffff04ff8202bfffff04ff15ff8080808080808080ffff01ff
    02ff16ffff04ff02ffff04ff0bffff04ff17ffff04ff8202bfffff04ff15ff80
    80808080808080ff0180ff80808080ffff01ff04ff2fffff01ff80ff80808080
    ff0180ffff04ffff01ffffff3f02ff04ff0101ffff822710ff02ff02ffff03ff
    05ffff01ff02ff3affff04ff02ffff04ff0dffff04ffff0bff2affff0bff2cff
    1480ffff0bff2affff0bff2affff0bff2cff3c80ff0980ffff0bff2aff0bffff
    0bff2cff8080808080ff8080808080ffff010b80ff0180ffff02ffff03ff17ff
    ff01ff04ffff04ff10ffff04ffff0bff81a7ffff02ff3effff04ff02ffff04ff
    ff04ff2fffff04ffff04ff05ffff04ffff05ffff14ffff12ff47ff0b80ff1280
    80ffff04ffff04ff05ff8080ff80808080ff808080ff8080808080ff808080ff
    ff02ff16ffff04ff02ffff04ff05ffff04ff0bffff04ff37ffff04ff2fff8080
    808080808080ff8080ff0180ffff0bff2affff0bff2cff1880ffff0bff2affff
    0bff2affff0bff2cff3c80ff0580ffff0bff2affff02ff3affff04ff02ffff04
    ff07ffff04ffff0bff2cff2c80ff8080808080ffff0bff2cff8080808080ff02
    ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff3effff04ff02ffff04ff
    09ff80808080ffff02ff3effff04ff02ffff04ff0dff8080808080ffff01ff0b
    ffff0101ff058080ff0180ff018080
    "
);

/// This is the puzzle hash of the [NFT1 royalty transfer](https://chialisp.com/nfts) puzzle.
pub const NFT_ROYALTY_TRANSFER_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    025dee0fb1e9fa110302a7e9bfb6e381ca09618e2778b0184fa5c6b275cfce1f
    "
));

/// This is the puzzle reveal of the [NFT1 metadata updater](https://chialisp.com/nfts) puzzle.
pub const NFT_METADATA_UPDATER_PUZZLE: [u8; 241] = hex!(
    "
    ff02ffff01ff04ffff04ffff02ffff03ffff22ff27ff3780ffff01ff02ffff03
    ffff21ffff09ff27ffff01826d7580ffff09ff27ffff01826c7580ffff09ff27
    ffff01758080ffff01ff02ff02ffff04ff02ffff04ff05ffff04ff27ffff04ff
    37ff808080808080ffff010580ff0180ffff010580ff0180ffff04ff0bff8080
    80ffff01ff808080ffff04ffff01ff02ffff03ff05ffff01ff02ffff03ffff09
    ff11ff0b80ffff01ff04ffff04ff0bffff04ff17ff198080ff0d80ffff01ff04
    ff09ffff02ff02ffff04ff02ffff04ff0dffff04ff0bffff04ff17ff80808080
    80808080ff0180ff8080ff0180ff018080
    "
);

/// This is the puzzle hash of the [NFT1 metadata updater](https://chialisp.com/nfts) puzzle.
pub const NFT_METADATA_UPDATER_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    fe8a4b4e27a2e29a4d3fc7ce9d527adbcaccbab6ada3903ccf3ba9a769d2d78b
    "
));

/// This is the puzzle reveal of the [NFT1 intermediate launcher](https://chialisp.com/nfts) puzzle.
pub const NFT_INTERMEDIATE_LAUNCHER_PUZZLE: [u8; 65] = hex!(
    "
    ff02ffff01ff04ffff04ff04ffff04ff05ffff01ff01808080ffff04ffff04ff
    06ffff04ffff0bff0bff1780ff808080ff808080ffff04ffff01ff333cff0180
    80
    "
);

/// This is the puzzle hash of the [NFT1 intermediate launcher](https://chialisp.com/nfts) puzzle.
pub const NFT_INTERMEDIATE_LAUNCHER_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    7a32d2d9571d3436791c0ad3d7fcfdb9c43ace2b0f0ff13f98d29f0cc093f445
    "
));

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_puzzle_hash;

    #[test]
    fn puzzle_hashes() {
        assert_puzzle_hash!(NFT_STATE_LAYER_PUZZLE => NFT_STATE_LAYER_PUZZLE_HASH);
        assert_puzzle_hash!(NFT_OWNERSHIP_LAYER_PUZZLE => NFT_OWNERSHIP_LAYER_PUZZLE_HASH);
        assert_puzzle_hash!(NFT_ROYALTY_TRANSFER_PUZZLE => NFT_ROYALTY_TRANSFER_PUZZLE_HASH);
        assert_puzzle_hash!(NFT_METADATA_UPDATER_PUZZLE => NFT_METADATA_UPDATER_PUZZLE_HASH);
        assert_puzzle_hash!(NFT_INTERMEDIATE_LAUNCHER_PUZZLE => NFT_INTERMEDIATE_LAUNCHER_PUZZLE_HASH);
    }
}
