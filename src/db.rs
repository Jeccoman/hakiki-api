//! Database bootstrap: build the connection pool and run migrations on start.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

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
