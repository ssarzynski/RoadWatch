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
