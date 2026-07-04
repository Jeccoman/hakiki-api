//! Background webhook delivery worker with retries.

use axum::http::HeaderMap;
use chrono::{Duration as ChronoDuration, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use crate::domain::Environment;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

pub fn start_webhook_worker(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = process_due_deliveries(&state).await {
                tracing::error!("webhook worker error: {}", e);
            }
            sleep(Duration::from_secs(5)).await; // poll interval
        }
    });
}

async fn process_due_deliveries(state: &AppState) -> Result<(), anyhow::Error> {
    // Find deliveries that are due
    let rows: Vec<(Uuid, Uuid, String, String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, webhook_endpoint_id, event_type, payload::text, COALESCE(signature, ''), status, COALESCE(last_error, '')
        FROM webhook_deliveries
        WHERE (next_attempt_at IS NULL OR next_attempt_at <= now())
          AND status IN ('pending', 'retrying')
          AND attempts < max_attempts
        ORDER BY created_at
        LIMIT 20
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    for (delivery_id, endpoint_id, event_type, payload_str, _sig, _status, _err) in rows {
        // Load endpoint
        let endpoint_row: Option<(String, String, bool)> = sqlx::query_as(
            "SELECT url, secret, active FROM webhook_endpoints WHERE id = $1",
        )
        .bind(endpoint_id)
        .fetch_optional(&state.pool)
        .await?;

        if let Some((url, secret, active)) = endpoint_row {
            if !active {
                continue;
            }

            let client = Client::new();
            let event_id = Uuid::new_v4().to_string();
            let timestamp = Utc::now().timestamp().to_string();

            // Compute signature: t=<ts>,v1=<hmac of ts.payload>
            let payload_bytes = payload_str.as_bytes();
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
            mac.update(timestamp.as_bytes());
            mac.update(b".");
            mac.update(payload_bytes);
            let signature = hex::encode(mac.finalize().into_bytes());

            let signature_header = format!("t={},v1={}", timestamp, signature);

            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", "application/json".parse().unwrap());
            headers.insert("Hakiki-Signature", signature_header.parse().unwrap());
            headers.insert("Hakiki-Event-Id", event_id.parse().unwrap());
            headers.insert("Hakiki-Event-Type", event_type.parse().unwrap());

            // Record attempt start
            let attempt_num = sqlx::query_scalar::<_, i32>(
                "SELECT COALESCE(attempts,0) + 1 FROM webhook_deliveries WHERE id = $1",
            )
            .bind(delivery_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(1);

            sqlx::query(
                "UPDATE webhook_deliveries SET status='sending', attempts=$1, last_attempt_at=now() WHERE id=$2",
            )
            .bind(attempt_num)
            .bind(delivery_id)
            .execute(&state.pool)
            .await?;

            match client.post(&url).headers(headers).body(payload_str.clone()).send().await {
                Ok(resp) => {
                    let status_code = resp.status().as_u16() as i32;
                    let body = resp.text().await.unwrap_or_default();

                    if (200..300).contains(&status_code) {
                        sqlx::query(
                            r#"
                            UPDATE webhook_deliveries
                            SET status='delivered', delivered_at=now(), response_code=$1, response_body=$2
                            WHERE id=$3
                            "#,
                        )
                        .bind(status_code)
                        .bind(&body)
                        .bind(delivery_id)
                        .execute(&state.pool)
                        .await?;

                        sqlx::query(
                            "INSERT INTO webhook_attempts (id, delivery_id, attempt_number, status_code, response_body, attempted_at)
                             VALUES ($1,$2,$3,$4,$5,now())",
                        )
                        .bind(Uuid::new_v4())
                        .bind(delivery_id)
                        .bind(attempt_num)
                        .bind(status_code)
                        .bind(&body)
                        .execute(&state.pool)
                        .await?;
                    } else {
                        schedule_retry(state, delivery_id, attempt_num, Some(format!("HTTP {}", status_code))).await?;
                    }
                }
                Err(e) => {
                    schedule_retry(state, delivery_id, attempt_num, Some(e.to_string())).await?;
                }
            }
        }
    }
    Ok(())
}

async fn schedule_retry(
    state: &AppState,
    delivery_id: Uuid,
    attempt: i32,
    err: Option<String>,
) -> Result<(), sqlx::Error> {
    let backoff_secs = 5 * (1 << (attempt.min(5) as u32)); // simple exponential
    let next = Utc::now() + ChronoDuration::seconds(backoff_secs as i64);

    let err_clone = err.clone();
    sqlx::query(
        r#"
        UPDATE webhook_deliveries
        SET status = 'retrying',
            next_attempt_at = $1,
            last_error = $2
        WHERE id = $3
        "#,
    )
    .bind(next)
    .bind(&err_clone)
    .bind(delivery_id)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "INSERT INTO webhook_attempts (id, delivery_id, attempt_number, error, attempted_at)
         VALUES ($1,$2,$3,$4,now())",
    )
    .bind(Uuid::new_v4())
    .bind(delivery_id)
    .bind(attempt)
    .bind(&err)
    .execute(&state.pool)
    .await?;

    Ok(())
}