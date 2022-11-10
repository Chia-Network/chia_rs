Library providing building blocks for a Chia wallet.

BIP39 mnemonic handling:

```
fn entropy_to_mnemonic(entropy: &[u8; 32]) -> String
fn mnemonic_to_entropy(mnemonic: &str) -> Result<[u8; 32], Error>
fn entropy_to_seed(entropy: &[u8; 32]) -> [u8; 64]
```

SecretKey

```
impl SecretKey {
    pub fn from_seed(seed: &[u8; 64]) -> SecretKey
    pub fn from_bytes(bytes: &[u8; 32]) -> Option<SecretKey>
    pub fn to_bytes(&self) -> [u8; 32]

    pub fn public_key(&self) -> PublicKey

    pub fn derive_unhardened(&self, idx: u32) -> SecretKey
    pub fn derive_hardened(&self, idx: u32) -> SecretKey
}
```

PublicKey

```
impl PublicKey {
    pub fn from_bytes(bytes: &[u8; 48]) -> Option<PublicKey>
    pub fn to_bytes(&self) -> [u8; 48]
    pub fn derive_unhardened(&self, idx: u32) -> PublicKey
}
```

Unhardened Key derivation (`Key` can be both a secret- or public key)

```
fn master_to_wallet_unhardened_intermediate<Key: DerivableKey>(key: &Key) -> Key
fn master_to_wallet_unhardened<Key: DerivableKey>(key: &Key, idx: u32) -> Key

```

Hardened key derivation (only SecretKey)

```
fn master_to_wallet_hardened_intermediate(key: &SecretKey) -> SecretKey
fn master_to_wallet_hardened(key: &SecretKey, idx: u32) -> SecretKey
fn master_to_pool_singleton(key: &SecretKey, pool_wallet_idx: u32) -> SecretKey
fn master_to_pool_authentication(key: &SecretKey, pool_wallet_idx: u32, idx: u32) -> SecretKey
```

Signature

```
impl Signature {
    pub fn from_bytes(buf: &[u8; 96]) -> Option<Signature>
    pub fn to_bytes(&self) -> [u8; 96]
    pub fn aggregate(&mut self, sig: &Signature)
}

impl Default for Signature {
    fn default() -> Self
}
```

sign and verify (using the Augmented scheme)

```
pub fn sign<Msg: AsRef<[u8]>>(sk: &SecretKey, msg: Msg) -> Signature
pub fn aggregate<Sig: Borrow<Signature>, I>(sigs: I) -> Signature
    where I: IntoIterator<Item = Sig>
pub fn verify<Msg: AsRef<[u8]>>(sig: &Signature, key: &PublicKey, msg: Msg) -> bool
pub fn aggregate_verify<Pk: Borrow<PublicKey>, Msg: Borrow<[u8]>, I>(sig: &Signature, data: I) -> bool
    where I: IntoIterator<Item = (Pk, Msg)>
```
