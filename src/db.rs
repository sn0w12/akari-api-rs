use sqlx::postgres::PgPoolOptions;

pub type DbPool = sqlx::PgPool;

pub async fn init_pool(database_url: &str, max_connections: u32) -> Result<DbPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await
}
