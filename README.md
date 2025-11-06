Create a simple README.md:

# auth_middleware

An Actix Web middleware for authenticating requests using a remote API.

## 🚀 Features
- Extracts Authorization header or cookie
- Calls a configurable remote API to verify the user
- Attaches authenticated user info to request extensions

## 🧩 Example

```rust
use actix_web::{App, HttpServer, HttpResponse, web};
use auth_middleware::AuthMiddleware;

async fn index() -> HttpResponse {
    HttpResponse::Ok().body("Welcome to protected route!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(AuthMiddleware)
            .route("/", web::get().to(index))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

🔧 Environment

Set AUTH_API_URL to your authentication service endpoint:

AUTH_API_URL=http://localhost:8080/api/profile

📜 License

MIT


---

## ⚖️ 5. `LICENSE`

Use MIT (common for Rust libraries):

```text
MIT License

Copyright (c) 2025 David Macharia

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction...

🧠 6. Initialize Git
cd auth_middleware
git init
git add .
git commit -m "Initial commit: Auth middleware for Actix Web"

🚀 7. Test locally

Before publishing, run:

cargo check
cargo test
cargo doc --open

🪶 8. Publish to crates.io

Log in with your GitHub account:

cargo login


(You’ll get your API token from https://crates.io/me
)

Publish the crate:

cargo publish


That’s it! 🎉
Your middleware will now be live on crates.io and installable via:

[dependencies]
auth_middleware = "0.1"


Would you like me to add a test example showing how another crate can mock a request and check that the middleware inserts the user properly?