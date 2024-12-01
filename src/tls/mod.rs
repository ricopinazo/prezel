use account::{create_new_account, persist_credentials, read_account};
use certificate::{persist_certificate, read_stored_certificate, TlsCertificate};
use instant_acme::LetsEncrypt;
use log::info;
use registration::generate_tls_certificate;

use crate::conf::Conf;

mod account;
pub(crate) mod certificate;
mod registration;

pub(crate) async fn get_or_generate_tls_certificate(conf: &Conf) -> TlsCertificate {
    match read_stored_certificate() {
        Some(certificate) if certificate.hostname == conf.hostname => certificate,
        _ => {
            info!("stored certificate is not valid, requesting a new one");
            let account = match read_account().await {
                Ok(account) => account,
                Err(_) => {
                    let (account, credentials) =
                        create_new_account(LetsEncrypt::Production.url()).await;
                    persist_credentials(&credentials).await;
                    account
                }
            };
            let certificate = generate_tls_certificate(account, conf).await;
            persist_certificate(&certificate);
            certificate
        }
    }
}

#[cfg(test)]
mod tls_tests {
    use instant_acme::LetsEncrypt;

    use crate::{
        conf::Conf,
        tls::{account::create_new_account, registration::generate_tls_certificate},
    };

    #[tokio::test]
    async fn test_registration() {
        let conf = Conf::read();
        let (account, _credentials) = create_new_account(LetsEncrypt::Staging.url()).await;
        let certificate = generate_tls_certificate(account, &conf).await;
        dbg!(certificate);
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
