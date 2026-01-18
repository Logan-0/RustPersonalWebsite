mod auth;
mod config;
mod db;
mod downloads;
mod handlers;
mod mail;

use actix_cors::Cors;
use actix_files::Files;
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{cookie::Key, middleware, web, App, HttpServer};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = match Config::from_env() {
        Ok(cfg) => Arc::new(cfg),
        Err(e) => {
            tracing::warn!("Api Functionality Limited: {}", e);
            Arc::new(Config::default())
        }
    };

    // Initialize database
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data.db".to_string());
    let db_pool = match db::init_pool(&database_url).await {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to initialize database: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Database initialization failed"));
        }
    };

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080u16);

    info!("Starting Actix-web server on http://localhost:{}", port);

    let config_data = web::Data::from(config);
    let db_data = web::Data::new(db_pool);

    // Session secret key - in production, load from env
    let secret_key = Key::from(
        std::env::var("SESSION_SECRET")
            .unwrap_or_else(|_| "0".repeat(64))
            .as_bytes(),
    );

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .supports_credentials()
            .max_age(31536000);

        App::new()
            .app_data(config_data.clone())
            .app_data(db_data.clone())
            .wrap(middleware::Logger::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                    .cookie_secure(false) // Set to true in production with HTTPS
                    .build(),
            )
            .wrap(cors)
            // Auth routes
            .route("/api/auth/login", web::post().to(auth::login))
            .route("/api/auth/logout", web::post().to(auth::logout))
            .route("/api/auth/me", web::get().to(auth::me))
            // Download routes
            .route("/api/files", web::get().to(downloads::list_files))
            .route("/api/files/token", web::post().to(downloads::generate_token))
            .route("/downloads/token/{token}", web::get().to(downloads::download_by_token))
            .route("/downloads/public/{path:.*}", web::get().to(downloads::download_public))
            .route("/email", web::post().to(handlers::send_email))
            // Serve static files from client build directory
            .service(Files::new("/static", "../client/leptosUI/dist"))
            // SPA fallback - serve index.html for all other routes
            .default_service(web::route().to(handlers::spa_fallback))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
