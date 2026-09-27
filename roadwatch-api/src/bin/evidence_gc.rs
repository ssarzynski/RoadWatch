use roadwatch_api::db;
use roadwatch_image_ingestion::storage::LocalEvidenceStore;
use sqlx::postgres::PgPoolOptions;
use std::{
    env,
    sync::Arc,
    time::{Duration, SystemTime},
};

const QUARANTINE_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const ORPHAN_GRACE: Duration = Duration::from_secs(24 * 60 * 60);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let apply = env::args().any(|a| a == "--apply");
    let database_url = env::var("DATABASE_URL")?;
    let quarantine = env::var("ROADWATCH_QUARANTINE_DIR")?;
    let sanitized = env::var("ROADWATCH_SANITIZED_DIR")?;

    let pool = PgPoolOptions::new().max_connections(2).connect(&database_url).await?;
    let store = Arc::new(LocalEvidenceStore::new(quarantine, sanitized)?);
    let now = SystemTime::now();

    println!("RoadWatch evidence GC mode: {}", if apply { "APPLY" } else { "DRY RUN" });

    for (key, modified) in store.list_quarantine()? {
        if now.duration_since(modified).unwrap_or_default() < QUARANTINE_RETENTION {
            continue;
        }

        let referenced = db::quarantine_key_is_referenced(&pool, &key).await?;
        if referenced {
            println!("expired quarantine referenced: {key}");
            if apply {
                // Clear only DB references old enough under the retention policy,
                // then re-check before deleting the private object.
                db::clear_expired_quarantine_reference(&pool, &key).await?;
                if db::quarantine_key_is_referenced(&pool, &key).await? {
                    println!("KEEP quarantine still referenced: {key}");
                    continue;
                }
                store.delete_quarantine(&key)?;
                println!("DELETED expired quarantine: {key}");
            }
        } else if apply {
            store.delete_quarantine(&key)?;
            println!("DELETED orphan quarantine: {key}");
        } else {
            println!("would delete orphan quarantine: {key}");
        }
    }

    for (key, modified) in store.list_sanitized()? {
        if now.duration_since(modified).unwrap_or_default() < ORPHAN_GRACE {
            continue;
        }
        if db::sanitized_key_is_referenced(&pool, &key).await? {
            continue;
        }
        if apply {
            store.delete_sanitized(&key)?;
            println!("DELETED orphan sanitized object: {key}");
        } else {
            println!("would delete orphan sanitized object: {key}");
        }
    }

    Ok(())
}
