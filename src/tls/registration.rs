use http::StatusCode;
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, NewOrder, OrderStatus,
};
use log::info;
use pingora::tls;
use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

use super::certificate::TlsCertificate;
use crate::conf::Conf;

#[derive(Deserialize)]
struct Ready {
    ready: bool,
}

pub(crate) async fn generate_tls_certificate(account: Account, conf: &Conf) -> TlsCertificate {
    info!("generating new TLS certificate");
    let Conf {
        token,
        hostname,
        coordinator,
    } = conf;
    let wildcard_domain = format!("*.{hostname}");
    let mut order = account
        .new_order(&NewOrder {
            identifiers: &[Identifier::Dns(wildcard_domain.clone())],
        })
        .await
        .unwrap();

    assert_eq!(order.state().status, OrderStatus::Pending);

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
        .iter()
        .find(|c| c.r#type == ChallengeType::Dns01)
        .ok_or_else(|| anyhow::anyhow!("no dns01 challenge found"))
        .unwrap();
    // let Identifier::Dns(identifier) = &authorization.identifier;
    let challenge_response = order.key_authorization(challenge).dns_value();

    dbg!(&challenge);
    dbg!(&challenge_response);

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

    // Challenge ready!
    order.set_challenge_ready(&challenge.url).await.unwrap();

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
        if tries > 5 {
            panic!("order is not ready after 5 tries");
        }
    }

    dbg!(order.state());
    assert_eq!(order.state().status, OrderStatus::Ready);

    // If the order is ready, we can provision the certificate.
    // Use the rcgen library to create a Certificate Signing Request.

    let mut params = CertificateParams::new(vec![wildcard_domain]).unwrap();
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
    TlsCertificate {
        cert: tls::x509::X509::from_pem(cert.as_bytes()).unwrap(),
        key: tls::pkey::PKey::private_key_from_der(key_der.as_slice()).unwrap(),
        hostname: hostname.clone(),
    }
}
