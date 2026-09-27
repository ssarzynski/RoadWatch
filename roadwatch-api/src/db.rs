use sqlx::{PgPool, Row};
use uuid::Uuid;

pub const DUPLICATE_CANDIDATE_RADIUS_M: f64 = 30.0;

#[derive(Debug, Clone)]
pub struct CameraRow {
    pub camera_id: Uuid,
    pub latitude: f64,
    pub longitude: f64,
    pub road_name: Option<String>,
    pub bearing_degrees: Option<i16>,
    pub camera_function: String,
    pub status: String,
    pub presence_evidence_score: i16,
    pub classification_evidence_score: i16,
}

#[derive(Debug, Clone)]
pub struct NewReport<'a> {
    pub latitude: f64,
    pub longitude: f64,
    pub bearing_degrees: Option<i16>,
    pub claimed_function: &'a str,
    pub claimed_manufacturer: Option<&'a str>,
    pub observed_at: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct StoredReport {
    pub report_id: Uuid,
    pub candidate_camera_ids: Vec<Uuid>,
}

pub async fn store_report(pool: &PgPool, report: NewReport<'_>) -> Result<StoredReport, sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Candidate lookup only. A spatial hit never auto-merges or verifies records.
    let candidates = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT camera_id
        FROM cameras
        WHERE status NOT IN ('removed')
          AND ST_DWithin(
            location,
            ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography,
            $3
          )
        ORDER BY ST_Distance(
            location,
            ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography
        )
        LIMIT 10
        "#,
    )
    .bind(report.longitude)
    .bind(report.latitude)
    .bind(DUPLICATE_CANDIDATE_RADIUS_M)
    .fetch_all(&mut *tx)
    .await?;

    // We intentionally do not attach the report to a canonical camera yet.
    // Candidate resolution belongs to the verification/deduplication workflow.
    let report_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO reports (
          camera_id, observed_location, claimed_function,
          claimed_manufacturer, bearing_degrees, observed_at
        )
        VALUES (
          NULL,
          ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography,
          $3::camera_function,
          $4,
          $5,
          CASE WHEN $6::text IS NULL THEN NULL ELSE $6::timestamptz END
        )
        RETURNING report_id
        "#,
    )
    .bind(report.longitude)
    .bind(report.latitude)
    .bind(report.claimed_function)
    .bind(report.claimed_manufacturer)
    .bind(report.bearing_degrees)
    .bind(report.observed_at)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(StoredReport { report_id, candidate_camera_ids: candidates })
}


#[derive(Debug)]
pub struct NewEvidence<'a> {
    pub report_id: Uuid,
    pub quarantine_object_key: Option<&'a str>,
    pub sanitized_object_key: Option<&'a str>,
    pub original_sha256: &'a str,
    pub sanitized_sha256: &'a str,
    pub perceptual_hash: u64,
    pub ingestion_decision: &'a str,
    pub eligible_for_scoring: bool,
}



#[derive(Debug)]
pub struct ContributorRiskFacts {
    pub reports_last_hour: i64,
    pub prior_latitude: Option<f64>,
    pub prior_longitude: Option<f64>,
    pub prior_received_unix: Option<i64>,
}

pub async fn contributor_risk_facts(
    pool: &PgPool,
    contributor_token_hash: &str,
) -> Result<ContributorRiskFacts, sqlx::Error> {
    let reports_last_hour = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT count(*)::bigint
        FROM reports
        WHERE contributor_token_hash = $1
          AND created_at >= now() - interval '1 hour'
        "#,
    )
    .bind(contributor_token_hash)
    .fetch_one(pool)
    .await?;

    let prior = sqlx::query_as::<_, (f64, f64, i64)>(
        r#"
        SELECT
          ST_Y(observed_location::geometry) AS latitude,
          ST_X(observed_location::geometry) AS longitude,
          extract(epoch from created_at)::bigint AS received_unix
        FROM reports
        WHERE contributor_token_hash = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(contributor_token_hash)
    .fetch_optional(pool)
    .await?;

    Ok(match prior {
        Some((lat, lon, ts)) => ContributorRiskFacts {
            reports_last_hour,
            prior_latitude: Some(lat),
            prior_longitude: Some(lon),
            prior_received_unix: Some(ts),
        },
        None => ContributorRiskFacts {
            reports_last_hour,
            prior_latitude: None,
            prior_longitude: None,
            prior_received_unix: None,
        },
    })
}

pub async fn exact_image_replay_exists(
    pool: &PgPool,
    original_sha256: &str,
    exclude_report_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM evidence
          WHERE original_sha256 = $1
            AND report_id <> $2
        )
        "#,
    )
    .bind(original_sha256)
    .bind(exclude_report_id)
    .fetch_one(pool)
    .await
}

pub async fn recent_perceptual_hashes(
    pool: &PgPool,
    exclude_report_id: Uuid,
    limit: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT perceptual_hash
        FROM evidence
        WHERE report_id <> $1
          AND perceptual_hash IS NOT NULL
          AND created_at >= now() - interval '30 days'
        ORDER BY created_at DESC
        LIMIT $2
        "#,
    )
    .bind(exclude_report_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn quarantine_key_is_referenced(pool: &PgPool, key: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM evidence WHERE quarantine_object_key = $1)"
    )
    .bind(key)
    .fetch_one(pool)
    .await
}

pub async fn sanitized_key_is_referenced(pool: &PgPool, key: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM evidence WHERE sanitized_object_key = $1)"
    )
    .bind(key)
    .fetch_one(pool)
    .await
}

pub async fn clear_expired_quarantine_reference(
    pool: &PgPool,
    key: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE evidence
        SET quarantine_object_key = NULL
        WHERE quarantine_object_key = $1
          AND created_at < now() - interval '7 days'
        "#,
    )
    .bind(key)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn insert_image_evidence(pool: &PgPool, e: NewEvidence<'_>) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO evidence (
          report_id, evidence_type, quarantine_object_key, sanitized_object_key,
          original_sha256, sanitized_sha256, perceptual_hash,
          ingestion_decision, eligible_for_scoring, metadata_stripped
        )
        VALUES ($1, 'photo', $2, $3, $4, $5, $6, $7, $8, true)
        RETURNING evidence_id
        "#,
    )
    .bind(e.report_id)
    .bind(e.quarantine_object_key)
    .bind(e.sanitized_object_key)
    .bind(e.original_sha256)
    .bind(e.sanitized_sha256)
    .bind(format!("{:016x}", e.perceptual_hash))
    .bind(e.ingestion_decision)
    .bind(e.eligible_for_scoring)
    .fetch_one(pool)
    .await
}

pub async fn nearby(pool: &PgPool, lat: f64, lon: f64, radius_m: u32) -> Result<Vec<CameraRow>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT camera_id,
          ST_Y(location::geometry) AS latitude,
          ST_X(location::geometry) AS longitude,
          road_name, bearing_degrees,
          camera_function::text AS camera_function,
          status::text AS status,
          presence_evidence_score, classification_evidence_score
        FROM cameras
        WHERE status NOT IN ('removed', 'removed_pending')
          AND ST_DWithin(
            location,
            ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography,
            $3
          )
        ORDER BY ST_Distance(
          location,
          ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography
        )
        LIMIT 500
        "#,
    )
    .bind(lon).bind(lat).bind(radius_m as f64)
    .fetch_all(pool).await?;

    rows.into_iter().map(|row| Ok(CameraRow {
        camera_id: row.try_get("camera_id")?,
        latitude: row.try_get("latitude")?,
        longitude: row.try_get("longitude")?,
        road_name: row.try_get("road_name")?,
        bearing_degrees: row.try_get("bearing_degrees")?,
        camera_function: row.try_get("camera_function")?,
        status: row.try_get("status")?,
        presence_evidence_score: row.try_get("presence_evidence_score")?,
        classification_evidence_score: row.try_get("classification_evidence_score")?,
    })).collect()
}
