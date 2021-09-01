use std::sync::Arc;

use quinn::{Certificate, ClientConfig, ClientConfigBuilder};
use rustls::{RootCertStore, ServerCertVerified, ServerCertVerifier};
#[allow(deprecated)]
use webpki::DNSNameRef;

pub enum Verification {
    CertificateAuthorities(Vec<Certificate>),
    Skip,
}

impl Verification {
    pub(crate) fn apply(
        self,
        mut config: ClientConfigBuilder,
    ) -> Result<ClientConfig, webpki::Error> {
        Ok(match self {
            Verification::CertificateAuthorities(certificates) => {
                for certificate in certificates {
                    config.add_certificate_authority(certificate)?;
                }
                config.build()
            }
            Verification::Skip => {
                let mut config = config.build();
                Arc::get_mut(&mut config.crypto)
                    .unwrap()
                    .dangerous()
                    .set_certificate_verifier(Arc::new(SkipVerificationVerifier));
                config
            }
        })
    }
}

struct SkipVerificationVerifier;

impl ServerCertVerifier for SkipVerificationVerifier {
    fn verify_server_cert(
        &self,
        _roots: &RootCertStore,
        _presented_certs: &[rustls::Certificate],
        _dns_name: DNSNameRef,
        _ocsp_response: &[u8],
    ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
        Ok(ServerCertVerified::assertion())
    }
}
