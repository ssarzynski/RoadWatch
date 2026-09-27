use sqlx::{PgPool, Row};
use uuid::Uuid;

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

pub async fn nearby(pool: &PgPool, lat: f64, lon: f64, radius_m: u32) -> Result<Vec<CameraRow>, sqlx::Error> {
    // PostGIS MakePoint is X,Y = longitude,latitude.
    let rows = sqlx::query(
        r#"
        SELECT
          camera_id,
          ST_Y(location::geometry) AS latitude,
          ST_X(location::geometry) AS longitude,
          road_name,
          bearing_degrees,
          camera_function::text AS camera_function,
          status::text AS status,
          presence_evidence_score,
          classification_evidence_score
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
    .bind(lon)
    .bind(lat)
    .bind(radius_m as f64)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(CameraRow {
                camera_id: row.try_get("camera_id")?,
                latitude: row.try_get("latitude")?,
                longitude: row.try_get("longitude")?,
                road_name: row.try_get("road_name")?,
                bearing_degrees: row.try_get("bearing_degrees")?,
                camera_function: row.try_get("camera_function")?,
                status: row.try_get("status")?,
                presence_evidence_score: row.try_get("presence_evidence_score")?,
                classification_evidence_score: row.try_get("classification_evidence_score")?,
            })
        })
        .collect()
}
