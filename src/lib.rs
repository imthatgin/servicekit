pub mod auth;
pub mod config;
pub mod server;

pub use auth::{CookieAuth, CurrentUser, add_auth, require_auth};
pub use config::Config;
pub use server::Server;

use axum::Router;

/// Sets up the initial state for the service.
pub fn init() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
}

/// Starts the axum server.
pub async fn serve(app: Router) -> std::io::Result<()> {
    let config = Config::from_env();
    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("starting server on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
