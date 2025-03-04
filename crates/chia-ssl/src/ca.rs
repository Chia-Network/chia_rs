use rcgen::{Certificate, CertificateParams, KeyPair};
use std::sync::LazyLock;

pub const CHIA_CA_KEY: &str = include_str!("../chia_ca.key");
pub const CHIA_CA_CRT: &str = include_str!("../chia_ca.crt");

pub static CHIA_CA: LazyLock<Certificate> = LazyLock::new(load_ca_cert);
pub static CHIA_CA_KEY_PAIR: LazyLock<KeyPair> =
    LazyLock::new(|| KeyPair::from_pem(CHIA_CA_KEY).expect("could not load CA keypair"));

fn load_ca_cert() -> Certificate {
    let params =
        CertificateParams::from_ca_cert_pem(CHIA_CA_CRT).expect("could not create CA params");
    params
        .self_signed(&CHIA_CA_KEY_PAIR)
        .expect("could not create certificate")
}
