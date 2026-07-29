use sqlx::postgres::PgPoolOptions;

pub type DbPool = sqlx::PgPool;

pub async fn init_pool(database_url: &str) -> Result<DbPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await
}
