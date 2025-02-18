use std::future::{ready, Ready};

use actix_web::{
    dev::Payload, error::ErrorUnauthorized, http::header::Header, web::Data, Error, FromRequest,
    HttpRequest,
};
use actix_web_httpauth::headers::authorization::{Authorization, Bearer};

use crate::tokens::{decode_token, Role, TokenClaims};

use super::AppState;

#[derive(Debug, Clone)]
pub struct AnyRole(TokenClaims);

impl AnyRole {
    fn validate(req: &HttpRequest) -> Result<Self, Error> {
        Authorization::<Bearer>::parse(req)
            .map_err(|_error| {
                // let bearer = req
                //     .app_data::<Config>()
                //     .map(|config| config.0.clone())
                //     .unwrap_or_default();
                // AuthenticationError::new(bearer) // TODO: use this instead?
                ErrorUnauthorized("missing token")
            })
            .and_then(|auth| {
                let scheme = auth.into_scheme();
                let token = scheme.token();
                // TODO: get secret from app state, which can be accessed from the req object
                let data = req.app_data::<Data<AppState>>().unwrap();
                let claims = decode_token(token, &data.secret)
                    .map_err(|_error| ErrorUnauthorized("invalid token"))?;
                Ok(Self(claims))
            })
    }
}

impl FromRequest for AnyRole {
    type Future = Ready<Result<Self, Self::Error>>;
    type Error = Error;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> <Self as FromRequest>::Future {
        ready(AnyRole::validate(req))
    }
}

#[derive(Debug, Clone)]
pub struct AdminRole;

impl FromRequest for AdminRole {
    type Future = Ready<Result<Self, Self::Error>>;
    type Error = Error;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> <Self as FromRequest>::Future {
        ready(AnyRole::validate(req).and_then(|claims| {
            if claims.0.role == Role::admin {
                Ok(Self)
            } else {
                Err(ErrorUnauthorized("not enough permissions"))
            }
        }))
    }
}
