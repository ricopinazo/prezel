use std::{future::Future, pin::Pin};

use actix_web::{dev::ServiceRequest, error::ErrorUnauthorized, Error};
use actix_web_httpauth::{extractors::bearer::BearerAuth, middleware::HttpAuthentication};

use crate::tokens::{decode_token, Role};

type Output = Pin<Box<dyn Future<Output = Result<ServiceRequest, (Error, ServiceRequest)>>>>;

pub(super) fn auth(
    secret: String,
) -> HttpAuthentication<BearerAuth, impl Fn(ServiceRequest, BearerAuth) -> Output> {
    let auth = HttpAuthentication::bearer(move |req, credentials| -> Output {
        let secret = secret.clone();
        Box::pin(async move {
            if let Ok(claims) = decode_token(credentials.token(), &secret) {
                if ["/apps", "/apps/{name}"].contains(&req.path()) {
                    Ok(req)
                } else if (claims.role == Role::owner) {
                    Ok(req)
                } else {
                    Err((ErrorUnauthorized("not enough permissions"), req))
                }
            } else {
                Err((ErrorUnauthorized("invalid token"), req))
            }
        })
    });
    auth
}
