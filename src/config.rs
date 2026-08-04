use clap::Parser;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub api_key: String,
    pub encryption_key: [u8; 32],
    pub mal_client_id: String,
    pub db_max_connections: u32,
    /// VAPID subject (e.g. "mailto:admin@example.com"); empty when unset.
    pub vapid_subject: String,
    /// VAPID public key, base64url 65-byte uncompressed P-256 point; empty when unset.
    pub vapid_public_key: String,
    /// VAPID private key, base64url 32-byte P-256 scalar; empty when unset.
    pub vapid_private_key: String,
}

impl Config {
    pub fn from_env() -> Self {
        fn env(key: &str) -> String {
            std::env::var(key).unwrap_or_else(|_| panic!("{} must be set", key))
        }

        let enc_key = env("ENCRYPTION_KEY");
        let mut key = [0u8; 32];
        let bytes = enc_key.as_bytes();
        if bytes.len() != 32 {
            panic!("ENCRYPTION_KEY must be exactly 32 bytes");
        }
        key.copy_from_slice(&bytes[..32]);

        Self {
            database_url: env("DATABASE_URL"),
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap(),
            api_key: env("API_KEY"),
            encryption_key: key,
            mal_client_id: env("MAL_CLIENT_ID"),
            db_max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            vapid_subject: std::env::var("WEBPUSH_SUBJECT").unwrap_or_default(),
            vapid_public_key: std::env::var("VAPID_PUBLIC_KEY").unwrap_or_default(),
            vapid_private_key: std::env::var("VAPID_PRIVATE_KEY").unwrap_or_default(),
        }
    }

    pub fn from_cli() -> Self {
        let cli = Cli::parse();

        let mut key = [0u8; 32];
        let bytes = cli.encryption_key.as_bytes();
        if bytes.len() != 32 {
            panic!("ENCRYPTION_KEY must be exactly 32 bytes");
        }
        key.copy_from_slice(&bytes[..32]);

        Self {
            database_url: cli.database_url,
            host: cli.host,
            port: cli.port,
            api_key: cli.api_key,
            encryption_key: key,
            mal_client_id: cli.mal_client_id,
            db_max_connections: cli.db_max_connections,
            vapid_subject: cli.vapid_subject,
            vapid_public_key: cli.vapid_public_key,
            vapid_private_key: cli.vapid_private_key,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "akari-api-rs", version, about = "Akari manga catalogue API")]
struct Cli {
    #[arg(short, long, env = "DATABASE_URL")]
    database_url: String,

    #[arg(long, env = "HOST", default_value = "0.0.0.0")]
    host: String,

    #[arg(short = 'P', long, env = "PORT", default_value_t = 3000)]
    port: u16,

    #[arg(long, env = "API_KEY")]
    api_key: String,

    #[arg(long, env = "ENCRYPTION_KEY")]
    encryption_key: String,

    #[arg(long, env = "MAL_CLIENT_ID")]
    mal_client_id: String,

    #[arg(long, env = "DB_MAX_CONNECTIONS", default_value_t = 20)]
    db_max_connections: u32,

    #[arg(long, env = "WEBPUSH_SUBJECT", default_value = "")]
    vapid_subject: String,

    #[arg(long, env = "VAPID_PUBLIC_KEY", default_value = "")]
    vapid_public_key: String,

    #[arg(long, env = "VAPID_PRIVATE_KEY", default_value = "")]
    vapid_private_key: String,
}
