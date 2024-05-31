pub trait DerivableKey {
    #[must_use]
    fn derive_unhardened(&self, idx: u32) -> Self;
}
