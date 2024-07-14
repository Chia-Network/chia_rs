use native_tls::{Identity, TlsConnector};

pub fn create_tls_connector(
    cert_pem: &[u8],
    key_pem: &[u8],
) -> Result<TlsConnector, native_tls::Error> {
    TlsConnector::builder()
        .identity(Identity::from_pkcs8(cert_pem, key_pem)?)
        .danger_accept_invalid_certs(true)
        .build()
}
