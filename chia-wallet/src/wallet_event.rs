#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletEvent {
    SyncStatusUpdate {
        derivation_index: u32,
        is_synced: bool,
    },
}
