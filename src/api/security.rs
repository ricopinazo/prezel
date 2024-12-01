use std::future::{self, Ready};

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    HttpResponse,
};
use futures::future::LocalBoxFuture;

use crate::conf::Conf;

use super::ErrorResponse;

pub(super) const API_KEY_NAME: &str = "X-API-Key";

pub(super) struct RequireApiKey;

impl<S> Transform<S, ServiceRequest> for RequireApiKey
where
    S: Service<
        ServiceRequest,
        Response = ServiceResponse<actix_web::body::BoxBody>,
        Error = actix_web::Error,
    >,
    S::Future: 'static,
{
    type Response = ServiceResponse<actix_web::body::BoxBody>;
    type Error = actix_web::Error;
    type Transform = ApiKeyMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        let Conf { token, .. } = Conf::read();
        future::ready(Ok(ApiKeyMiddleware {
            service,
            api_key: token,
        }))
    }
}

pub(super) struct ApiKeyMiddleware<S> {
    service: S,
    api_key: String,
}

impl<S> Service<ServiceRequest> for ApiKeyMiddleware<S>
where
    S: Service<
        ServiceRequest,
        Response = ServiceResponse<actix_web::body::BoxBody>,
        Error = actix_web::Error,
    >,
    S::Future: 'static,
{
    type Response = ServiceResponse<actix_web::body::BoxBody>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, actix_web::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let response = |req: ServiceRequest, response: HttpResponse| -> Self::Future {
            Box::pin(async { Ok(req.into_response(response)) })
        };

        match req.headers().get(API_KEY_NAME) {
            Some(key) if key != &self.api_key => {
                // TODO: avoid these early returns, make the Box::ping generic at the bottom
                return response(
                    req,
                    HttpResponse::Unauthorized().json(ErrorResponse::Unauthorized(String::from(
                        "incorrect api key",
                    ))),
                );
            }
            None => {
                return response(
                    req,
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::Unauthorized(String::from("missing api key"))),
                );
            }
            _ => (), // just passthrough
        }

        let future = self.service.call(req);

        Box::pin(async move {
            let response = future.await?;
            Ok(response)
        })
    }
}
