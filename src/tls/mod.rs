use std::{
    collections::HashMap,
    fmt::Debug,
    ops::Deref,
    sync::{Arc, RwLock},
};

use account::{create_new_account, persist_credentials, read_account};
use certificate::TlsCertificate;
use instant_acme::{Account, ChallengeType, LetsEncrypt};
use log::info;
use registration::{
    generate_certificate_and_persist, read_or_generate_default_certificate_and_persist,
};

use crate::conf::Conf;

mod account;
pub(crate) mod certificate;
mod registration;

#[derive(Clone, Debug)]
pub(crate) enum TlsState {
    // Pending,
    Challenge {
        challenge_file: String,
        challenge_content: String,
    },
    Ready(TlsCertificate),
}

// TODO: move this to utils file
struct IgnoreDebug<T>(T);
impl<T> Debug for IgnoreDebug<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ignored field")
    }
}
impl<T> Deref for IgnoreDebug<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: Clone> Clone for IgnoreDebug<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> From<T> for IgnoreDebug<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CertificateStore {
    account: IgnoreDebug<Account>,
    default: TlsCertificate,
    domains: Arc<RwLock<HashMap<String, TlsState>>>,
}

impl CertificateStore {
    pub(crate) fn get_domain(&self, domain: &str) -> Option<TlsState> {
        self.domains.read().unwrap().get(domain).cloned()
    }

    pub(crate) fn has_domain(&self, domain: &str) -> bool {
        self.domains.read().unwrap().contains_key(domain)
    }

    pub(crate) fn get_default_certificate(&self) -> TlsCertificate {
        self.default.clone()
    }

    pub(crate) async fn load(conf: &Conf) -> Self {
        let account = match read_account().await {
            Ok(account) => account,
            Err(_) => {
                let (account, credentials) =
                    create_new_account(LetsEncrypt::Production.url()).await;
                persist_credentials(&credentials).await;
                account
            }
        };
        let default =
            read_or_generate_default_certificate_and_persist(&account, conf.clone()).await;
        Self {
            account: account.into(),
            default,
            domains: Default::default(),
        }
    }

    pub(crate) fn insert_domain(&self, domain: String) {
        let domains = self.domains.clone();
        let account = self.account.clone();
        let cloned_domain = domain.clone();

        tokio::spawn(async move {
            let certificate = generate_certificate_and_persist(
                &account,
                domain.clone(),
                ChallengeType::Http01,
                |challenge| {
                    let challenge_file = challenge.get_http_file_name();
                    let challenge_content = challenge.get_http_file_content();
                    domains.write().unwrap().insert(
                        cloned_domain,
                        TlsState::Challenge {
                            challenge_file,
                            challenge_content,
                        },
                    );
                    async {}
                },
            )
            .await;
            domains
                .write()
                .unwrap()
                .insert(domain, TlsState::Ready(certificate));
        });
    }
}

// pub(crate) async fn get_or_generate_tls_certificate(conf: &Conf) -> TlsCertificate {
//     match read_stored_certificate() {
//         Some(certificate) if certificate.hostname == conf.hostname => certificate,
//         _ => {
//             info!("stored certificate is not valid, requesting a new one");
//             let account = match read_account().await {
//                 Ok(account) => account,
//                 Err(_) => {
//                     let (account, credentials) =
//                         create_new_account(LetsEncrypt::Production.url()).await;
//                     persist_credentials(&credentials).await;
//                     account
//                 }
//             };
//             let certificate = generate_tls_certificate(account, conf).await;
//             persist_certificate(&certificate);
//             certificate
//         }
//     }
// }

#[cfg(test)]
mod tls_tests {
    use instant_acme::LetsEncrypt;

    use crate::{conf::Conf, tls::account::create_new_account};

    #[tokio::test]
    async fn test_registration() {
        let conf = Conf::read();
        let (account, _credentials) = create_new_account(LetsEncrypt::Staging.url()).await;
        // let certificate = generate_tls_certificate(account, &conf).await;
        // dbg!(certificate);
    }

    #[tokio::test]
    async fn test_account() {
        // TODO: re-enable this test!
        // let account = get_saved_account().await;
        // assert!(account.is_err());

        // get_saved_account_or_new(LetsEncrypt::Staging.url()).await;

        // let account = get_saved_account().await;
        // assert!(account.is_ok());
    }
}
