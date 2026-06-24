#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub graphql_path: String,
    pub playground_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            graphql_path: std::env::var("GRAPHQL_PATH")
                .unwrap_or_else(|_| "/graphql".into()),
            playground_path: std::env::var("PLAYGROUND_PATH")
                .unwrap_or_else(|_| "/playground".into()),
        }
    }
}
