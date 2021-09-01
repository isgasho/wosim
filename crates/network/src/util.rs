use std::{
    fs::read,
    io,
    path::{Path, PathBuf},
};

use quinn::{Certificate, CertificateChain, ParseError, PrivateKey};
use rcgen::generate_simple_self_signed;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FromPemError {
    #[error("could not read certificate chain file '{1}'")]
    ReadCertificateChain(#[source] io::Error, PathBuf),
    #[error("could not parse certificate chain")]
    ParseCertificateChain(#[source] ParseError),
    #[error("could not read private key file '{1}'")]
    ReadPrivateKey(#[source] io::Error, PathBuf),
    #[error("could not parse private key")]
    ParsePrivateKey(#[source] ParseError),
}

pub fn self_signed() -> (CertificateChain, PrivateKey) {
    let cert = generate_simple_self_signed(["localhost".to_owned()]).unwrap();
    let der = cert.serialize_private_key_der();
    let private_key = PrivateKey::from_der(&der).unwrap();
    let der = cert.serialize_der().unwrap();
    let cert = Certificate::from_der(&der).unwrap();
    let certificate_chain = CertificateChain::from_certs(vec![cert]);
    (certificate_chain, private_key)
}

pub fn from_pem(
    certificate_chain: impl AsRef<Path>,
    private_key: impl AsRef<Path>,
) -> Result<(CertificateChain, PrivateKey), FromPemError> {
    let certificate_chain = read(certificate_chain.as_ref()).map_err(|e| {
        FromPemError::ReadCertificateChain(e, certificate_chain.as_ref().to_path_buf())
    })?;
    let certificate_chain = CertificateChain::from_pem(&certificate_chain)
        .map_err(FromPemError::ParseCertificateChain)?;
    let private_key = read(private_key.as_ref())
        .map_err(|e| FromPemError::ReadPrivateKey(e, private_key.as_ref().to_path_buf()))?;
    let private_key = PrivateKey::from_pem(&private_key).map_err(FromPemError::ParsePrivateKey)?;
    Ok((certificate_chain, private_key))
}
