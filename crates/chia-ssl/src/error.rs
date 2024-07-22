use time::error::ComponentRange;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("{0}")]
    KeyGen(#[from] rsa::Error),

    #[error("{0}")]
    Pkcs8(#[from] rsa::pkcs8::Error),

    #[error("{0}")]
    CertGen(#[from] rcgen::Error),

    #[error("{0}")]
    DateRange(#[from] ComponentRange),
}
