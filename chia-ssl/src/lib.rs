use lazy_static::lazy_static;
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, KeyPair, SanType};
use rsa::{
    pkcs8::{EncodePrivateKey, LineEnding},
    RsaPrivateKey,
};
use std::fmt;
use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time};

pub const CHIA_CA_KEY: &str = include_str!("../chia_ca.key");
pub const CHIA_CA_CRT: &str = include_str!("../chia_ca.crt");

lazy_static! {
    pub static ref CHIA_CA: Certificate = load_ca_cert();
}

fn load_ca_cert() -> Certificate {
    let key_pair = KeyPair::from_pem(CHIA_CA_KEY).expect("could not load CA keypair");
    let params = CertificateParams::from_ca_cert_pem(CHIA_CA_CRT, key_pair)
        .expect("could not create CA params");
    Certificate::from_params(params).expect("could not create certificate")
}

#[derive(Debug)]
pub struct ChiaSsl {
    pub cert_pem: String,
    pub key_pem: String,
}

#[derive(Debug)]
pub enum Error {
    KeyError { reason: String },
    CertError { reason: String },
    DateRangeError { reason: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyError { reason }
            | Self::CertError { reason }
            | Self::DateRangeError { reason } => write!(f, "{}", reason),
        }
    }
}

impl std::error::Error for Error {}

pub fn create_chia_ssl() -> Result<ChiaSsl, Error> {
    let mut rng = rand::thread_rng();

    let key = RsaPrivateKey::new(&mut rng, 2048).map_err(|error| Error::KeyError {
        reason: error.to_string(),
    })?;

    let key_pem = key
        .to_pkcs8_pem(LineEnding::default())
        .map_err(|error| Error::KeyError {
            reason: error.to_string(),
        })?
        .to_string();

    let mut params = CertificateParams::default();

    params.alg = &rcgen::PKCS_RSA_SHA256;
    params.key_pair = Some(
        KeyPair::from_pem(&key_pem).map_err(|error| Error::KeyError {
            reason: error.to_string(),
        })?,
    );

    let mut subject = DistinguishedName::new();
    subject.push(DnType::CommonName, "Chia");
    subject.push(DnType::OrganizationName, "Chia");
    subject.push(DnType::OrganizationalUnitName, "Organic Farming Division");
    params.distinguished_name = subject;

    params.subject_alt_names = vec![SanType::DnsName("chia.net".to_string())];

    params.not_before = OffsetDateTime::now_utc() - Duration::DAY;
    params.not_after = PrimitiveDateTime::new(
        Date::from_calendar_date(2100, Month::August, 2).map_err(|error| {
            Error::DateRangeError {
                reason: error.to_string(),
            }
        })?,
        Time::MIDNIGHT,
    )
    .assume_utc();

    let cert = Certificate::from_params(params).map_err(|error| Error::CertError {
        reason: error.to_string(),
    })?;

    let cert_pem = cert
        .serialize_pem_with_signer(&CHIA_CA)
        .map_err(|error| Error::CertError {
            reason: error.to_string(),
        })?;

    Ok(ChiaSsl { cert_pem, key_pem })
}
