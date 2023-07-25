#[derive(Debug, Clone)]
pub enum WalletEvent {
    DidConfirmed { did_id: [u8; 32] },
    NftConfirmed { nft_id: [u8; 32] },
    CatDiscovered { asset_id: [u8; 32] },
}
