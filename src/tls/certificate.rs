use std::{
    fs::{self, create_dir_all},
    path::PathBuf,
};

use pingora::tls;

use crate::paths::get_container_root;

#[derive(Debug, Clone)]
pub(crate) struct TlsCertificate {
    pub(crate) domain: String,
    pub(crate) cert: String,
    pub(crate) key: String,
}

impl TlsCertificate {
    pub(crate) fn load_from_disk(domain: String) -> anyhow::Result<Self> {
        let cert = get_cert_path(&domain);
        tls::x509::X509::from_pem(&fs::read(&cert)?)?;

        let key = get_key_path(&domain);
        tls::pkey::PKey::private_key_from_pem(&fs::read(&key)?)?;

        Ok(Self {
            domain,
            cert: cert.to_str().unwrap().to_owned(),
            key: key.to_str().unwrap().to_owned(),
        })
    }

    // pub(crate) fn write_to_disk(&self) -> anyhow::Result<()> {
    //     let cert = self.cert.to_pem()?;
    //     fs::write(get_cert_path(&self.domain), cert)?;

    //     let key = self.key.private_key_to_pem_pkcs8()?;
    //     fs::write(get_key_path(&self.domain), key)?;

    //     Ok(())
    // }
}

pub(crate) fn write_certificate_to_disk(
    domain: &str,
    cert: tls::x509::X509,
    key: tls::pkey::PKey<tls::pkey::Private>,
) -> anyhow::Result<()> {
    fs::write(get_cert_path(domain), cert.to_pem()?)?;
    fs::write(get_key_path(domain), key.private_key_to_pem_pkcs8()?)?;
    Ok(())
}

fn get_cert_path(domain: &str) -> PathBuf {
    get_domain_path(domain).join("cert.pem")
}

fn get_key_path(domain: &str) -> PathBuf {
    get_domain_path(domain).join("key.pem")
}

fn get_domain_path(domain: &str) -> PathBuf {
    let path = get_container_root().join("certs").join(domain);
    create_dir_all(&path).unwrap();
    path
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
