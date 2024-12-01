use std::{fs, path::PathBuf};

use log::info;
use pingora::tls;
use serde::{Deserialize, Serialize};

use crate::paths::get_container_root;

#[derive(Debug, Clone)]
pub(crate) struct TlsCertificate {
    pub(crate) cert: tls::x509::X509,
    pub(crate) key: tls::pkey::PKey<tls::pkey::Private>,
    pub(crate) hostname: String,
}

#[derive(Serialize, Deserialize)]
struct StoredTlsCertificate {
    pub(crate) cert: Vec<u8>,
    pub(crate) key: Vec<u8>,
    pub(crate) hostname: String,
}

impl Serialize for TlsCertificate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let credentials = StoredTlsCertificate {
            cert: self.cert.to_pem().unwrap(),
            key: self.key.private_key_to_der().unwrap(),
            hostname: self.hostname.to_owned(),
        };
        credentials.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TlsCertificate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let credentials = StoredTlsCertificate::deserialize(deserializer)?;
        Ok(Self {
            cert: tls::x509::X509::from_pem(&credentials.cert).unwrap(),
            key: tls::pkey::PKey::private_key_from_der(&credentials.key).unwrap(),
            hostname: credentials.hostname,
        })
    }
}

fn certificate_path() -> PathBuf {
    get_container_root().join("acme-credentials")
}

pub(crate) fn read_stored_certificate() -> Option<TlsCertificate> {
    info!("reading stored certificate");
    let bytes = fs::read(certificate_path()).ok()?;
    bincode::deserialize(&bytes).ok()
}

pub(crate) fn persist_certificate(certificate: &TlsCertificate) {
    let bytes = bincode::serialize(certificate).unwrap();
    fs::write(certificate_path(), &bytes).unwrap();
}

#[cfg(test)]
mod test_certificate {
    use super::read_stored_certificate;

    #[test]
    fn test_cert() {
        let cert = read_stored_certificate().unwrap();

        // dbg!(cert.cert.authority_key_id().unwrap());

        // this is None
        // for point in cert.cert.crl_distribution_points().unwrap() {
        //     dbg!(point
        //         .distpoint()
        //         .unwrap()
        //         .fullname()
        //         .unwrap()
        //         .into_iter()
        //         .collect::<Vec<_>>());
        // }

        for resp in cert.cert.ocsp_responders().unwrap() {
            let str: String = resp.chars().collect();
            dbg!(str);
        }

        for desc in cert.cert.authority_info().unwrap() {
            dbg!(desc.location());
            dbg!(desc.method());
        }
        dbg!(cert.cert.issuer_name());
        // dbg!(cert.cert.digest().unwrap());
    }
}
