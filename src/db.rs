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

/// Ensures a demo organization exists for the marketing site demo.
pub async fn ensure_demo_organization(pool: &PgPool) -> Result<Uuid, sqlx::Error> {
    let org_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    sqlx::query(
        r#"
        INSERT INTO organizations (id, name, slug, status)
        VALUES ($1, 'Hakiki Demo', 'demo', 'active')
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(org_id)
    .execute(pool)
    .await?;

    // Ensure a demo member
    sqlx::query(
        r#"
        INSERT INTO organization_members (id, organization_id, email, role)
        VALUES ($1, $2, 'demo@hakiki.test', 'admin')
        ON CONFLICT (organization_id, email) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap())
    .bind(org_id)
    .execute(pool)
    .await?;

    Ok(org_id)
}

/// Idempotently register an API key (by name) and attach to demo org.
/// Used to seed the publishable demo key...
pub async fn upsert_api_key(pool: &PgPool, name: &str, raw_key: &str) -> Result<(), sqlx::Error> {
    let demo_org_id = ensure_demo_organization(pool).await?;

    sqlx::query(
        r#"
        INSERT INTO api_keys (id, organization_id, partner_name, key_hash, environment)
        VALUES ($1, $2, $3, $4, 'test')
        ON CONFLICT (key_hash) DO UPDATE SET organization_id = EXCLUDED.organization_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(demo_org_id)
    .bind(name)
    .bind(sha256_hex(raw_key))
    .execute(pool)
    .await?;
    Ok(())
}
