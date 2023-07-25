use chia_primitives::NftMetadata;

#[derive(Debug, Clone)]
pub struct NftMint {
    pub target_puzzle_hash: [u8; 32],
    pub royalty_puzzle_hash: [u8; 32],
    pub royalty_percentage: u16,
    pub metadata: NftMetadata,
}
