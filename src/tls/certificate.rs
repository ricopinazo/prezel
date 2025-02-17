use std::{
    fs::{self, create_dir_all, File},
    io,
    path::{Path, PathBuf},
};

use futures::{stream, StreamExt};
use pingora::tls;

use crate::paths::get_container_root;

#[derive(Debug, Clone)]
pub(crate) struct TlsCertificate {
    pub(crate) domain: String,
    pub(crate) cert: String,
    pub(crate) key: String,
    pub(crate) intermediates: Vec<tls::x509::X509>,
}

fn read_pem_from_path(path: &Path) -> anyhow::Result<tls::x509::X509> {
    Ok(tls::x509::X509::from_pem(&fs::read(&path)?)?)
}

async fn get_intermediate_pem_path(url: &str) -> anyhow::Result<tls::x509::X509> {
    let path = get_intermediate_path(url);
    match read_pem_from_path(&path) {
        Ok(cert) => Ok(cert),
        Err(_) => {
            let response = reqwest::get(url).await?;
            let content = response.bytes().await?;
            let cert = tls::x509::X509::from_der(&content)?;
            fs::write(&path, cert.to_pem()?)?;
            read_pem_from_path(&path)
        }
    }
}

impl TlsCertificate {
    pub(crate) async fn load_from_disk(domain: String) -> anyhow::Result<Self> {
        let key = get_key_path(&domain);
        tls::pkey::PKey::private_key_from_pem(&fs::read(&key)?)?;

        let cert = get_cert_path(&domain);
        let cert_content = tls::x509::X509::from_pem(&fs::read(&cert)?)?;
        let info = cert_content.authority_info().unwrap();
        let uris = info
            .iter()
            .filter(|access| matches!(access.method().nid().long_name(), Ok("CA Issuers")))
            .filter_map(|access| access.location().uri())
            .map(|uri| uri.to_owned());
        let intermediates = stream::iter(uris)
            .then(|uri| async move { get_intermediate_pem_path(&uri).await.unwrap() })
            .collect()
            .await;

        Ok(Self {
            domain,
            intermediates,
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

fn get_intermediate_path(uri: &str) -> PathBuf {
    let filename = uri.replace("/", "").replace(":", "") + ".pem";
    get_container_root().join("certs").join(&filename)
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
    use std::fs;

    use pingora::tls;

    use crate::tls::certificate::get_cert_path;

    #[test]
    fn test_cert() {
        let domain = "*.planned-platypus.018294.xyz";
        let path = get_cert_path(&domain);
        let cert = tls::x509::X509::from_pem(&fs::read(&path).unwrap()).unwrap();

        // dbg!(cert.authority_key_id().unwrap());

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

        for resp in cert.ocsp_responders().unwrap() {
            let str: String = resp.chars().collect();
            dbg!(str);
        }

        for desc in cert.authority_info().unwrap() {
            dbg!(desc.location());
            dbg!(desc.method());
            dbg!(desc.method().nid());
            dbg!(desc.method().nid().as_raw());
            dbg!(desc.method().nid().long_name());
            dbg!(desc.method().nid().short_name());
        }
        dbg!(cert.issuer_name());
        // dbg!(cert.cert.digest().unwrap());
    }
}
