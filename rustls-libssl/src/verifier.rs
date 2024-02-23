use core::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use openssl_sys::{
    X509_V_ERR_CERT_HAS_EXPIRED, X509_V_ERR_CERT_NOT_YET_VALID, X509_V_ERR_CERT_REVOKED,
    X509_V_ERR_HOSTNAME_MISMATCH, X509_V_ERR_INVALID_PURPOSE,
    X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT_LOCALLY, X509_V_ERR_UNSPECIFIED, X509_V_OK,
};

use rustls::{
    client::{
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        verify_server_cert_signed_by_trust_anchor, verify_server_name,
    },
    crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider},
    pki_types::{CertificateDer, ServerName, UnixTime},
    server::ParsedCertificate,
    CertificateError, DigitallySignedStruct, Error, RootCertStore, SignatureScheme,
};

use crate::VerifyMode;

/// This is a verifier that implements the selection of bad ideas from OpenSSL:
///
/// - that the SNI name and verified certificate server name are unrelated
/// - that the server name can be empty, and that implicitly disables hostname verification
/// - that the behaviour defaults to verifying nothing
#[derive(Debug)]
pub struct ServerVerifier {
    root_store: Arc<RootCertStore>,

    provider: Arc<CryptoProvider>,

    /// Expected server name.
    ///
    /// `None` means server name verification is disabled.
    verify_hostname: Option<ServerName<'static>>,

    mode: VerifyMode,

    last_result: AtomicI64,
}

impl ServerVerifier {
    pub fn new(
        root_store: Arc<RootCertStore>,
        provider: Arc<CryptoProvider>,
        mode: VerifyMode,
        hostname: &Option<ServerName<'static>>,
    ) -> Self {
        Self {
            root_store,
            provider,
            verify_hostname: hostname.clone(),
            mode,
            last_result: AtomicI64::new(X509_V_ERR_UNSPECIFIED as i64),
        }
    }

    pub fn last_result(&self) -> i64 {
        self.last_result.load(Ordering::Acquire)
    }

    fn verify_server_cert_inner(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<(), Error> {
        let end_entity = ParsedCertificate::try_from(end_entity)?;

        verify_server_cert_signed_by_trust_anchor(
            &end_entity,
            &self.root_store,
            intermediates,
            now,
            self.provider.signature_verification_algorithms.all,
        )?;

        if let Some(server_name) = &self.verify_hostname {
            verify_server_name(&end_entity, server_name)?;
        }

        Ok(())
    }
}

impl ServerCertVerifier for ServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _ignored_server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let result = self.verify_server_cert_inner(end_entity, intermediates, now);

        let openssl_rv = match &result {
            Ok(()) => X509_V_OK,
            Err(Error::InvalidCertificate(CertificateError::UnknownIssuer)) => {
                X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT_LOCALLY
            }
            Err(Error::InvalidCertificate(CertificateError::NotValidYet)) => {
                X509_V_ERR_CERT_NOT_YET_VALID
            }
            Err(Error::InvalidCertificate(CertificateError::Expired)) => {
                X509_V_ERR_CERT_HAS_EXPIRED
            }
            Err(Error::InvalidCertificate(CertificateError::Revoked)) => X509_V_ERR_CERT_REVOKED,
            Err(Error::InvalidCertificate(CertificateError::InvalidPurpose)) => {
                X509_V_ERR_INVALID_PURPOSE
            }
            Err(Error::InvalidCertificate(CertificateError::NotValidForName)) => {
                X509_V_ERR_HOSTNAME_MISMATCH
            }
            // TODO: more mappings can go here
            Err(_) => X509_V_ERR_UNSPECIFIED,
        };
        self.last_result.store(openssl_rv as i64, Ordering::Release);

        // Call it success if it succeeded, or the `mode` says not to care.
        if openssl_rv == X509_V_OK || !self.mode.client_must_verify_server() {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(result.unwrap_err())
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}