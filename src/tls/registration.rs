use http::StatusCode;
use instant_acme::{
    Account, AuthorizationStatus, Challenge, ChallengeType, Identifier, KeyAuthorization, NewOrder,
    Order, OrderStatus,
};
use log::info;
use pingora::tls;
use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use serde::Deserialize;
use std::{future::Future, sync::Arc, time::Duration};
use tokio::time::sleep;

use super::certificate::{write_certificate_to_disk, TlsCertificate};
use crate::conf::Conf;

pub(crate) async fn read_or_generate_default_certificate_and_persist(
    account: &Account,
    conf: Conf,
) -> TlsCertificate {
    let wildcard_domain = format!("*.{}", conf.hostname);
    generate_certificate_and_persist(account, wildcard_domain, ChallengeType::Dns01, |handle| {
        write_dns_challenge(handle, &conf)
    })
    .await
}

pub(crate) async fn generate_certificate_and_persist<
    O: Future<Output = ()>,
    F: FnOnce(Arc<ChallengeTask>) -> O,
>(
    account: &Account,
    domain: String,
    challenge_type: ChallengeType,
    handle_challenge: F,
) -> TlsCertificate {
    match TlsCertificate::load_from_disk(domain.clone()) {
        Ok(certificate) if certificate.domain == domain => certificate,
        _ => {
            let mut order = create_order(account, domain.clone()).await;
            if order.state().status == OrderStatus::Pending {
                let challenge = get_challenge(&mut order, challenge_type).await;
                handle_challenge(challenge.clone()).await;
                complete_challenge(&mut order, challenge.as_ref()).await;
            }
            aquire_certificate(order, domain.clone()).await;
            TlsCertificate::load_from_disk(domain).unwrap()
        }
    }
}

pub(crate) struct ChallengeTask {
    challenge: Challenge,
    key_authorization: KeyAuthorization,
}

impl ChallengeTask {
    pub(crate) fn get_dns_value(&self) -> String {
        self.key_authorization.dns_value()
    }

    pub(crate) fn get_http_file_name(&self) -> String {
        self.challenge.token.clone()
    }

    pub(crate) fn get_http_file_content(&self) -> String {
        self.key_authorization.as_str().to_owned()
    }
}

async fn create_order(account: &Account, domain: String) -> Order {
    account
        .new_order(&NewOrder {
            identifiers: &[Identifier::Dns(domain)],
        })
        .await
        .unwrap()
}

async fn get_challenge(order: &mut Order, challenge_type: ChallengeType) -> Arc<ChallengeTask> {
    let authorizations = order.authorizations().await.unwrap();
    let authorization = authorizations.into_iter().next().unwrap();

    // wait for the authorization to be pending
    while authorization.status != AuthorizationStatus::Pending {
        if authorization.status != AuthorizationStatus::Valid {
            panic!("Unrecognized authroization status");
        }
        sleep(Duration::from_secs(1)).await
    }

    let challenge = authorization
        .challenges
        .into_iter()
        .find(|c| c.r#type == challenge_type)
        .ok_or_else(|| anyhow::anyhow!("no dns01 challenge found"))
        .unwrap();
    let key_authorization = order.key_authorization(&challenge);
    ChallengeTask {
        challenge,
        key_authorization,
    }
    .into()
}

async fn complete_challenge(order: &mut Order, challenge: &ChallengeTask) {
    order
        .set_challenge_ready(&challenge.challenge.url)
        .await
        .unwrap();

    // Exponentially back off until the order becomes ready or invalid.
    let mut tries = 1u8;
    let mut delay = Duration::from_millis(250);
    loop {
        sleep(delay).await;
        let state = order.refresh().await.unwrap();
        if let OrderStatus::Ready | OrderStatus::Invalid = state.status {
            break;
        }

        delay *= 2;
        tries += 1;
        if tries > 10 {
            panic!("order is not ready after 5 tries");
        }
    }

    dbg!(order.state());
    assert_eq!(order.state().status, OrderStatus::Ready);
}

async fn aquire_certificate(mut order: Order, domain: String) {
    let mut params = CertificateParams::new(vec![domain.clone()]).unwrap();
    params.distinguished_name = DistinguishedName::new();
    let private_key = KeyPair::generate().unwrap();
    let csr = params.serialize_request(&private_key).unwrap();

    order.finalize(csr.der()).await.unwrap();
    let cert = loop {
        match order.certificate().await.unwrap() {
            Some(cert_chain_pem) => break cert_chain_pem,
            None => sleep(Duration::from_secs(1)).await,
        }
    };

    let key_der = private_key.serialize_der();
    write_certificate_to_disk(
        &domain,
        tls::x509::X509::from_pem(cert.as_bytes()).unwrap(),
        tls::pkey::PKey::private_key_from_der(key_der.as_slice()).unwrap(),
    )
    .unwrap();
}

///////////////////////////////////////////////////////////////////////////////////////////////////

// pub(crate) struct ChallengeHandle {
//     order: Order,
//     domain: String,
//     challenge: Challenge,
//     key_authorization: KeyAuthorization,
// }

// impl ChallengeHandle {
//     pub(crate) fn get_dns_value(&self) -> String {
//         self.key_authorization.dns_value()
//     }

//     pub(crate) fn get_http_file_name(&self) -> String {
//         self.challenge.token.clone()
//     }

//     pub(crate) fn get_http_file_content(&self) -> String {
//         self.key_authorization.as_str().to_owned()
//     }

//     // domain has to be a wildcard if it is for a dns challenge
//     pub(crate) async fn generate(
//         account: &Account,
//         domain: String,
//         challenge_type: ChallengeType,
//     ) -> Self {
//         info!("generating new TLS certificate");
//         let mut order = account
//             .new_order(&NewOrder {
//                 identifiers: &[Identifier::Dns(domain.clone())],
//             })
//             .await
//             .unwrap();

//         assert_eq!(order.state().status, OrderStatus::Pending);

//         let authorizations = order.authorizations().await.unwrap();
//         let authorization = authorizations.into_iter().next().unwrap();

//         // wait for the authorization to be pending
//         while authorization.status != AuthorizationStatus::Pending {
//             if authorization.status != AuthorizationStatus::Valid {
//                 panic!("Unrecognized authroization status");
//             }
//             sleep(Duration::from_secs(1)).await
//         }

//         let challenge = authorization
//             .challenges
//             .into_iter()
//             .find(|c| c.r#type == challenge_type)
//             .ok_or_else(|| anyhow::anyhow!("no dns01 challenge found"))
//             .unwrap();
//         let key_authorization = order.key_authorization(&challenge);
//         Self {
//             order,
//             domain,
//             challenge,
//             key_authorization,
//         }
//     }

//     pub(crate) async fn complete_and_persist(mut self) -> TlsCertificate {
//         // Challenge ready!
//         self.order
//             .set_challenge_ready(&self.challenge.url)
//             .await
//             .unwrap();

//         // Exponentially back off until the order becomes ready or invalid.
//         let mut tries = 1u8;
//         let mut delay = Duration::from_millis(250);
//         loop {
//             sleep(delay).await;
//             let state = self.order.refresh().await.unwrap();
//             if let OrderStatus::Ready | OrderStatus::Invalid = state.status {
//                 break;
//             }

//             delay *= 2;
//             tries += 1;
//             if tries > 5 {
//                 panic!("order is not ready after 5 tries");
//             }
//         }

//         dbg!(self.order.state());
//         assert_eq!(self.order.state().status, OrderStatus::Ready);

//         // If the order is ready, we can provision the certificate.
//         // Use the rcgen library to create a Certificate Signing Request.

//         let mut params = CertificateParams::new(vec![self.domain.clone()]).unwrap();
//         params.distinguished_name = DistinguishedName::new();
//         let private_key = KeyPair::generate().unwrap();
//         let csr = params.serialize_request(&private_key).unwrap();

//         self.order.finalize(csr.der()).await.unwrap();
//         let cert = loop {
//             match self.order.certificate().await.unwrap() {
//                 Some(cert_chain_pem) => break cert_chain_pem,
//                 None => sleep(Duration::from_secs(1)).await,
//             }
//         };

//         let key_der = private_key.serialize_der();
//         let certificate = TlsCertificate {
//             domain: self.domain,
//             cert: tls::x509::X509::from_pem(cert.as_bytes()).unwrap(),
//             key: tls::pkey::PKey::private_key_from_der(key_der.as_slice()).unwrap(),
//         };
//         certificate.write_to_disk().unwrap(); // TODO: remove this unwrap
//         certificate
//     }
// }

#[derive(Deserialize)]
struct Ready {
    ready: bool,
}

async fn write_dns_challenge(handle: Arc<ChallengeTask>, conf: &Conf) {
    let Conf {
        token,
        hostname,
        coordinator,
    } = conf;
    let challenge_response = handle.get_dns_value();

    // send request to the coordinator to setup DNS challenge
    let client = reqwest::Client::new();
    let url = format!("{coordinator}/api/instance/dns");
    let query = client
        .post(url)
        .header("X-API-Key", token)
        .header("X-Instance-ID", hostname)
        .body(challenge_response)
        .send();
    let response = query.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // wait until DNS is ready (both A and TXT record)
    loop {
        let client = reqwest::Client::new();
        let url = format!("{coordinator}/api/instance/dns");
        dbg!(&url);
        let response = client
            .get(url)
            .header("X-API-Key", token)
            .header("X-Instance-ID", hostname)
            // .body("some body")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let Ready { ready } = response.json().await.unwrap();
        if ready {
            break;
        } else {
            sleep(Duration::from_secs(5)).await;
            dbg!();
        }
    }
}
