pub mod db;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use roadwatch_image_ingestion::{evaluate_upload, IngestionDecision};
use roadwatch_image_ingestion::storage::LocalEvidenceStore;
use roadwatch_verification::risk::{self, PriorObservation, RiskInput};
use std::sync::Arc;
use sqlx::PgPool;
use uuid::Uuid;

pub const MAX_NEARBY_RADIUS_M: u32 = 5_000;

#[derive(Clone, Default)]
pub struct AppState {
    pub pool: Option<PgPool>,
    pub evidence_store: Option<Arc<LocalEvidenceStore>>,
    pub contributor_hmac_key: Option<Arc<Vec<u8>>>,
}

pub fn app() -> Router {
    app_with_state(AppState::default())
}


const CONTRIBUTOR_TOKEN_MIN_LEN: usize = 32;
const CONTRIBUTOR_TOKEN_MAX_LEN: usize = 128;

pub fn contributor_token_hash(raw: &str, key: &[u8]) -> Option<String> {
    if key.len() < 32
        || raw.len() < CONTRIBUTOR_TOKEN_MIN_LEN
        || raw.len() > CONTRIBUTOR_TOKEN_MAX_LEN
        || !raw.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(key).ok()?;
    mac.update(b"roadwatch-contributor-v1\0");
    mac.update(raw.as_bytes());
    Some(hex::encode(mac.finalize().into_bytes()))
}

pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/reports", post(create_report))
        .route("/v1/reports/{report_id}/evidence", post(upload_evidence))
        .route("/v1/cameras/nearby", get(nearby_cameras))
        .with_state(state)
}

async fn health() -> &'static str { "ok" }

#[derive(Debug, Deserialize)]
pub struct CreateReport {
    pub latitude: f64,
    pub longitude: f64,
    pub bearing_degrees: Option<u16>,
    pub claimed_function: Option<String>,
    pub claimed_manufacturer: Option<String>,
    pub observed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AcceptedReport {
    pub report_id: Uuid,
    pub status: &'static str,
    pub duplicate_candidate_ids: Vec<Uuid>,
}

async fn create_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(report): Json<CreateReport>,
) -> Result<(StatusCode, Json<AcceptedReport>), (StatusCode, Json<ApiError>)> {
    validate_coordinates(report.latitude, report.longitude)?;
    if report.bearing_degrees.is_some_and(|b| b > 359) {
        return Err(bad_request("bearing_degrees must be between 0 and 359"));
    }

    let claimed_function = report.claimed_function.as_deref().unwrap_or("unknown");
    const ALLOWED_FUNCTIONS: &[&str] = &[
        "alpr", "speed_enforcement", "red_light_enforcement", "traffic_monitoring",
        "tolling", "public_surveillance", "parking_municipal", "unknown",
    ];
    if !ALLOWED_FUNCTIONS.contains(&claimed_function) {
        return Err(bad_request("unsupported claimed_function"));
    }

    let contributor_hash = headers
        .get("x-roadwatch-contributor")
        .and_then(|v| v.to_str().ok())
        .and_then(|raw| {
            state
                .contributor_hmac_key
                .as_deref()
                .and_then(|key| contributor_token_hash(raw, key))
        });

    let report_risk = if let (Some(pool), Some(hash)) = (state.pool.as_ref(), contributor_hash.as_ref()) {
        match db::contributor_risk_facts(pool, hash).await {
            Ok(facts) => {
                let prior = match (facts.prior_latitude, facts.prior_longitude, facts.prior_received_unix) {
                    (Some(latitude), Some(longitude), Some(received_at)) =>
                        Some(PriorObservation { latitude, longitude, received_at }),
                    _ => None,
                };
                let received_at = sqlx::query_scalar::<_, i64>(
                    "SELECT extract(epoch from clock_timestamp())::bigint"
                ).fetch_one(pool).await.unwrap_or(0);
                Some(risk::assess(&RiskInput {
                    latitude: report.latitude,
                    longitude: report.longitude,
                    received_at,
                    reports_last_hour: facts.reports_last_hour.max(0) as u32,
                    exact_image_replay: false,
                    perceptual_image_replay: false,
                    correlated_source_count: 0,
                    prior: prior.as_ref(),
                }))
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Risk signals trigger review / loss of independent weight; they do not
    // automatically delete or reject an otherwise valid public observation.
    let _report_risk = report_risk;

    let Some(pool) = state.pool else {
        return Ok((StatusCode::ACCEPTED, Json(AcceptedReport {
            report_id: Uuid::new_v4(),
            status: "accepted_unverified_not_persisted",
            duplicate_candidate_ids: Vec::new(),
        })));
    };

    let stored = db::store_report(&pool, db::NewReport {
        contributor_token_hash: contributor_hash,
        latitude: report.latitude,
        longitude: report.longitude,
        bearing_degrees: report.bearing_degrees.map(|b| b as i16),
        claimed_function,
        claimed_manufacturer: report.claimed_manufacturer.as_deref(),
        observed_at: report.observed_at.as_deref(),
    }).await.map_err(|_| internal_error("database write failed"))?;

    Ok((StatusCode::ACCEPTED, Json(AcceptedReport {
        report_id: stored.report_id,
        status: "accepted_unverified",
        duplicate_candidate_ids: stored.candidate_camera_ids,
    })))
}

#[derive(Debug, Serialize)]
pub struct EvidenceUploadResponse {
    pub report_id: Uuid,
    pub decision: &'static str,
    pub eligible_for_scoring: bool,
    pub original_sha256: Option<String>,
    pub sanitized_sha256: Option<String>,
    pub perceptual_hash: Option<u64>,
    pub anomaly_flags: Vec<&'static str>,
}


fn perceptual_replay(current: u64, stored: &[String]) -> bool {
    stored.iter().any(|value| {
        u64::from_str_radix(value, 16)
            .map(|prior| roadwatch_image_ingestion::perceptual_distance(current, prior) <= 6)
            .unwrap_or(false)
    })
}

async fn upload_evidence(
    State(state): State<AppState>,
    Path(report_id): Path<Uuid>,
    body: axum::body::Bytes,
) -> (StatusCode, Json<EvidenceUploadResponse>) {
    let outcome = evaluate_upload(&body);

    let Some(image) = outcome.image else {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(EvidenceUploadResponse {
            report_id, decision: "rejected", eligible_for_scoring: false,
            original_sha256: None, sanitized_sha256: None, perceptual_hash: None,
            anomaly_flags: Vec::new(),
        }));
    };

    let decision = match outcome.decision {
        IngestionDecision::AcceptSanitized => "accepted_sanitized",
        IngestionDecision::Quarantine => "quarantined",
        IngestionDecision::Reject => "rejected",
    };
    let requested_eligibility = outcome.decision == IngestionDecision::AcceptSanitized;

    // Without both durable storage and DB, never claim evidence is scoring-eligible.
    let (Some(pool), Some(store)) = (state.pool, state.evidence_store) else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(EvidenceUploadResponse {
            report_id, decision: "storage_unavailable", eligible_for_scoring: false,
            original_sha256: Some(image.original_sha256),
            sanitized_sha256: Some(image.sanitized_sha256),
            perceptual_hash: Some(image.perceptual_hash),
            anomaly_flags: image.anomaly_flags,
        }));
    };

    let sanitized = match store.put_sanitized(&image.sanitized_sha256, &image.sanitized_bytes) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(EvidenceUploadResponse {
            report_id, decision: "storage_failed", eligible_for_scoring: false,
            original_sha256: Some(image.original_sha256),
            sanitized_sha256: Some(image.sanitized_sha256),
            perceptual_hash: Some(image.perceptual_hash),
            anomaly_flags: image.anomaly_flags,
        })),
    };

    let quarantine = if outcome.decision == IngestionDecision::Quarantine {
        match store.put_quarantine(&body) {
            Ok(v) => Some(v),
            Err(_) => {
                let _ = store.delete_sanitized(&sanitized.object_key);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(EvidenceUploadResponse {
                    report_id, decision: "quarantine_storage_failed", eligible_for_scoring: false,
                    original_sha256: Some(image.original_sha256),
                    sanitized_sha256: Some(image.sanitized_sha256),
                    perceptual_hash: Some(image.perceptual_hash),
                    anomaly_flags: image.anomaly_flags,
                }));
            }
        }
    } else { None };

    // Risk screening uses server-held history and stored evidence. It cannot
    // declare malice; it only controls independence/review disposition.
    let exact_replay = db::exact_image_replay_exists(&pool, &image.original_sha256, report_id)
        .await
        .unwrap_or(true);
    let stored_hashes = db::recent_perceptual_hashes(&pool, report_id, 500)
        .await
        .unwrap_or_default();
    let perceptual_replay = perceptual_replay(image.perceptual_hash, &stored_hashes);

    // Contributor correlation will become active once pseudonymous contributor
    // tokens are accepted and stored by the report endpoint. Until then we do
    // not invent a device identity or claim independence from device history.
    let risk = risk::assess(&RiskInput {
        latitude: 0.0,
        longitude: 0.0,
        received_at: 0,
        reports_last_hour: 0,
        exact_image_replay: exact_replay,
        perceptual_image_replay: perceptual_replay,
        correlated_source_count: 0,
        prior: None::<&PriorObservation>,
    });
    let risk_allows_scoring = risk.independent_weight_allowed;

    let inserted = db::insert_image_evidence(&pool, db::NewEvidence {
        report_id,
        quarantine_object_key: quarantine.as_ref().map(|o| o.object_key.as_str()),
        sanitized_object_key: Some(&sanitized.object_key),
        original_sha256: &image.original_sha256,
        sanitized_sha256: &image.sanitized_sha256,
        perceptual_hash: image.perceptual_hash,
        ingestion_decision: decision,
        eligible_for_scoring: requested_eligibility && risk_allows_scoring,
    }).await;

    if inserted.is_err() {
        // Compensation: quarantine names are unique and safe to remove. Sanitized
        // content-addressed objects may be shared, so retain them for later GC.
        if let Some(q) = quarantine {
            let _ = store.delete_quarantine(&q.object_key);
        }
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(EvidenceUploadResponse {
            report_id, decision: "database_failed", eligible_for_scoring: false,
            original_sha256: Some(image.original_sha256),
            sanitized_sha256: Some(image.sanitized_sha256),
            perceptual_hash: Some(image.perceptual_hash),
            anomaly_flags: image.anomaly_flags,
        }));
    }

    (StatusCode::ACCEPTED, Json(EvidenceUploadResponse {
        report_id, decision, eligible_for_scoring: requested_eligibility && risk_allows_scoring,
        original_sha256: Some(image.original_sha256),
        sanitized_sha256: Some(image.sanitized_sha256),
        perceptual_hash: Some(image.perceptual_hash),
        anomaly_flags: image.anomaly_flags,
    }))
}

#[derive(Debug, Deserialize)]
pub struct NearbyQuery {
    pub lat: f64,
    pub lon: f64,
    pub radius_m: u32,
}

#[derive(Debug, Serialize)]
pub struct NearbyResponse {
    pub radius_m: u32,
    pub cameras: Vec<PublicCamera>,
}

#[derive(Debug, Serialize)]
pub struct PublicCamera {
    pub camera_id: Uuid,
    pub latitude: f64,
    pub longitude: f64,
    pub road_name: Option<String>,
    pub bearing_degrees: Option<u16>,
    pub camera_function: String,
    pub status: String,
    pub presence_evidence_score: u8,
    pub classification_evidence_score: u8,
}

async fn nearby_cameras(
    State(state): State<AppState>,
    Query(query): Query<NearbyQuery>,
) -> Result<Json<NearbyResponse>, (StatusCode, Json<ApiError>)> {
    validate_coordinates(query.lat, query.lon)?;
    if query.radius_m == 0 || query.radius_m > MAX_NEARBY_RADIUS_M {
        return Err(bad_request("radius_m must be between 1 and 5000"));
    }

    let Some(pool) = state.pool else {
        return Ok(Json(NearbyResponse { radius_m: query.radius_m, cameras: Vec::new() }));
    };

    let rows = db::nearby(&pool, query.lat, query.lon, query.radius_m)
        .await
        .map_err(|_| internal_error("database query failed"))?;

    let cameras = rows.into_iter().map(|r| PublicCamera {
        camera_id: r.camera_id,
        latitude: r.latitude,
        longitude: r.longitude,
        road_name: r.road_name,
        bearing_degrees: r.bearing_degrees.and_then(|b| u16::try_from(b).ok()),
        camera_function: r.camera_function,
        status: r.status,
        presence_evidence_score: u8::try_from(r.presence_evidence_score).unwrap_or(0),
        classification_evidence_score: u8::try_from(r.classification_evidence_score).unwrap_or(0),
    }).collect();

    Ok(Json(NearbyResponse { radius_m: query.radius_m, cameras }))
}

#[derive(Debug, Serialize)]
pub struct ApiError { pub error: &'static str }

fn validate_coordinates(lat: f64, lon: f64) -> Result<(), (StatusCode, Json<ApiError>)> {
    if !lat.is_finite() || !lon.is_finite() || !(-90.0..=90.0).contains(&lat)
        || !(-180.0..=180.0).contains(&lon)
    {
        return Err(bad_request("invalid latitude or longitude"));
    }
    Ok(())
}

fn bad_request(message: &'static str) -> (StatusCode, Json<ApiError>) {
    (StatusCode::BAD_REQUEST, Json(ApiError { error: message }))
}

fn internal_error(message: &'static str) -> (StatusCode, Json<ApiError>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { error: message }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::{to_bytes, Body}, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_is_ok() {
        let response = app().oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_invalid_nearby_coordinates() {
        let response = app().oneshot(Request::builder().uri("/v1/cameras/nearby?lat=91&lon=0&radius_m=100").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn caps_nearby_radius() {
        let response = app().oneshot(Request::builder().uri("/v1/cameras/nearby?lat=33&lon=-84&radius_m=5001").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn no_database_never_fabricates_cameras() {
        let response = app().oneshot(Request::builder().uri("/v1/cameras/nearby?lat=33&lon=-84&radius_m=100").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["cameras"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn malformed_evidence_is_rejected_before_scoring() {
        let report_id = Uuid::new_v4();
        let response = app().oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/reports/{report_id}/evidence"))
                .header("content-type", "application/octet-stream")
                .body(Body::from("not-an-image"))
                .unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["eligible_for_scoring"], false);
        assert_eq!(value["decision"], "rejected");
    }

    #[tokio::test]
    async fn accepts_valid_report_without_verifying_it() {
        let body = serde_json::json!({"latitude":33.0,"longitude":-84.0,"bearing_degrees":90,"claimed_function":"unknown"});
        let response = app().oneshot(
            Request::builder().method("POST").uri("/v1/reports")
                .header("content-type","application/json")
                .body(Body::from(body.to_string())).unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["status"], "accepted_unverified_not_persisted");
    }
    #[test]
    fn contributor_token_is_hashed_and_raw_value_is_not_returned() {
        let raw = "A234567890123456789012345678901234567890123";
        let hash = contributor_token_hash(raw, &[7_u8; 32]).unwrap();
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, raw);
    }

    #[test]
    fn contributor_token_rejects_short_or_pathological_values() {
        assert!(contributor_token_hash("short", &[7_u8; 32]).is_none());
        assert!(contributor_token_hash(&"x".repeat(129), &[7_u8; 32]).is_none());
        assert!(contributor_token_hash("this token contains spaces and is long enough", &[7_u8; 32]).is_none());
    }


}
