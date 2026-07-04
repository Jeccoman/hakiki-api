//! Database bootstrap: build the connection pool and run migrations on start.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::sha256_hex;

pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    // Migrations are embedded from ./migrations at compile time, so the binary
    // is self-contained — no separate migration step to run on deploy.
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

/// Idempotently register an API key (by name). Used to seed the publishable
/// demo key on startup so the marketing-site demo works out of the box.
/// Real partner keys should be issued out-of-band, not via env.
pub async fn upsert_api_key(pool: &PgPool, name: &str, raw_key: &str) -> Result<(), sqlx::Error> {
    // Default demo key to test environment
    sqlx::query(
        "insert into api_keys (id, partner_name, key_hash, environment)
         values ($1, $2, $3, 'test')
         on conflict (key_hash) do nothing",
    )
    .bind(Uuid::new_v4())
    .bind(name)
    .bind(sha256_hex(raw_key))
    .execute(pool)
    .await?;
    Ok(())
}
