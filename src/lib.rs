use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
};
use futures::future::{ok, Ready, LocalBoxFuture};
use reqwest::Client;
use std::rc::Rc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::NaiveDateTime;
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

pub struct AuthMiddleware;

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
        })
    }
}

pub struct AuthMiddlewareImpl<S> {
    service: Rc<S>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
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

        Box::pin(async move {
            // 🔍 Try header first
            let mut token_opt: Option<String> = req
                .headers()
                .get("Authorization")
                .and_then(|hv| hv.to_str().ok())
                .map(|s| s.to_string());

            // 🍪 If no Authorization header, check cookie
            if token_opt.is_none() {
                if let Some(cookie) = req.cookie("Authorization") {
                    token_opt = Some(cookie.value().to_string());
                    println!("🍪 Found Authorization cookie");
                }
            }

            // ❌ Reject immediately if no token found
            let token = match token_opt {
                Some(t) => t,
                None => {
                    eprintln!("⚠️ No Authorization header or cookie found");
                    let response = HttpResponse::Unauthorized()
                        .body("Unauthorized: Missing Authorization token")
                        .map_into_right_body();
                    return Ok(req.into_response(response));
                }
            };

            // ✅ Validate token via remote API
            let client = Client::new();
            let api_url = env::var("AUTH_API_URL")
                .unwrap_or_else(|_| "http://localhost:8080/api/profile".to_string());

            println!("→ Calling AUTH API: {} | Token: {}", api_url, token);

            let user = match client
                .get(&api_url)
                .header("Authorization", token.clone())
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => match res.json::<RemoteUser>().await {
                    Ok(user) => {
                        println!("✅ Authenticated user: {:?}", user.username);
                        Some(user)
                    }
                    Err(err) => {
                        eprintln!("❌ Failed to parse user JSON: {:?}", err);
                        None
                    }
                },
                Ok(res) => {
                    eprintln!("❌ AUTH API error: {}", res.status());
                    None
                }
                Err(err) => {
                    eprintln!("❌ Failed to call AUTH API: {:?}", err);
                    None
                }
            };

            // ❌ Reject if user validation failed
            if user.is_none() {
                let response = HttpResponse::Unauthorized()
                    .body("Unauthorized: Invalid or expired token")
                    .map_into_right_body();
                return Ok(req.into_response(response));
            }

            // ✅ Attach Option<RemoteUser> to request extensions
            req.extensions_mut().insert(user);

            // Continue request
            let res = service.call(req).await?.map_into_left_body();
            Ok(res)
        })
    }
}
