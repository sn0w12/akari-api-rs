use std::net::SocketAddr;

use akari_api_rs::app::build_app;
use akari_api_rs::auth::AppState;
use akari_api_rs::config::Config;
use akari_api_rs::db::init_pool;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("akari_api_rs=info,tower_http=info")),
        )
        .init();

    let config = Config::from_cli();
    let pool = init_pool(&config.database_url, config.db_max_connections)
        .await
        .expect("Failed to connect to database");
    tracing::info!("Connected to database");

    let state = AppState {
        db: pool.clone(),
        config: config.clone(),
    };

    let app = build_app(state, true);

    let addr = SocketAddr::new(config.host.parse().expect("Invalid HOST"), config.port);
    tracing::info!("Starting server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server failed");
}
