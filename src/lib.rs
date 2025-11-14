use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
};
use futures::future::{ok, Ready, LocalBoxFuture};
use reqwest::Method;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;
use std::rc::Rc;
use std::env;
use actix_web::body::EitherBody;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteUser {
    pub id: String,
    pub created_at: Option<NaiveDateTime>,
    pub deleted_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
    pub enabled: Option<bool>,
    pub mobile: Option<String>,
    pub phone_verified: Option<bool>,
    pub username: String,
    pub referral_code_id: Option<String>,
    pub available_points: f64,
}

#[derive(Clone)]
pub struct AuthMiddleware {
    lax_paths: Vec<String>,
    lax_methods: Vec<Method>,
    continue_on_fail: bool,
}

impl AuthMiddleware {
    pub fn new(lax_paths: Vec<String>, lax_methods: Vec<Method>, continue_on_fail: bool) -> Self {
        Self { lax_paths, lax_methods, continue_on_fail }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareImpl<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthMiddlewareImpl {
            service: Rc::new(service),
            lax_paths: self.lax_paths.clone(),
            lax_methods: self.lax_methods.clone(),
            continue_on_fail: self.continue_on_fail,
        })
    }
}

pub struct AuthMiddlewareImpl<S> {
    service: Rc<S>,
    lax_paths: Vec<String>,
    lax_methods: Vec<Method>,
    continue_on_fail: bool,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareImpl<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let lax_paths = self.lax_paths.clone();
        let lax_methods = self.lax_methods.clone();
        let continue_on_fail = self.continue_on_fail;

        Box::pin(async move {
            let path = req.path();
            let method = req.method();

            // Skip auth if path or method matches lax rules
            let is_lax_path = lax_paths.iter().any(|p| path.starts_with(p));
            let is_lax_method = lax_methods.iter().any(|m| m == method);

            if is_lax_path || is_lax_method {
                return service.call(req).await.map(|res| res.map_into_left_body());
            }

            // Extract token from header or cookie
            let mut token_opt = req
                .headers()
                .get("Authorization")
                .and_then(|hv| hv.to_str().ok())
                .map(|s| s.to_string());

            if token_opt.is_none() {
                if let Some(cookie) = req.cookie("Authorization") {
                    token_opt = Some(cookie.value().to_string());
                }
            }

            let token = token_opt.unwrap_or_default();

            // Validate token via remote API
            let user: Option<RemoteUser> = if !token.is_empty() {
                let client = Client::new();
                let api_url = env::var("AUTH_API_URL")
                    .unwrap_or_else(|_| "http://localhost:8080/api/profile".to_string());

                match client.get(&api_url).header("Authorization", token.clone()).send().await {
                    Ok(res) if res.status().is_success() => res.json::<RemoteUser>().await.ok(),
                    _ => None,
                }
            } else {
                None
            };

            // If auth fails
            if user.is_none() && !continue_on_fail {
                let response = HttpResponse::Unauthorized()
                    .body("Unauthorized: Invalid or missing token")
                    .map_into_right_body();
                return Ok(req.into_response(response));
            }

            // Attach user info to request extensions
            req.extensions_mut().insert(user);

            // Continue request
            let res = service.call(req).await?.map_into_left_body();
            Ok(res)
        })
    }
}
