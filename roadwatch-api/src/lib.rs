use axum::{
    extract::Query,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_NEARBY_RADIUS_M: u32 = 5_000;

pub fn app() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/reports", post(create_report))
        .route("/v1/cameras/nearby", get(nearby_cameras))
}

async fn health() -> &'static str {
    "ok"
}

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
}

async fn create_report(
    Json(report): Json<CreateReport>,
) -> Result<(StatusCode, Json<AcceptedReport>), (StatusCode, Json<ApiError>)> {
    validate_coordinates(report.latitude, report.longitude)?;
    if report.bearing_degrees.is_some_and(|b| b > 359) {
        return Err(bad_request("bearing_degrees must be between 0 and 359"));
    }

    // Persistence and duplicate-candidate matching are added when PostGIS is wired in.
    // Acceptance is intentionally NOT equivalent to camera verification.
    Ok((
        StatusCode::ACCEPTED,
        Json(AcceptedReport {
            report_id: Uuid::new_v4(),
            status: "accepted_unverified",
        }),
    ))
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
    Query(query): Query<NearbyQuery>,
) -> Result<Json<NearbyResponse>, (StatusCode, Json<ApiError>)> {
    validate_coordinates(query.lat, query.lon)?;
    if query.radius_m == 0 || query.radius_m > MAX_NEARBY_RADIUS_M {
        return Err(bad_request("radius_m must be between 1 and 5000"));
    }

    // Empty until PostGIS persistence is connected. Public response deliberately
    // excludes contributor identifiers and private evidence locations.
    Ok(Json(NearbyResponse {
        radius_m: query.radius_m,
        cameras: Vec::new(),
    }))
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: &'static str,
}

fn validate_coordinates(
    lat: f64,
    lon: f64,
) -> Result<(), (StatusCode, Json<ApiError>)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_is_ok() {
        let response = app()
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_invalid_nearby_coordinates() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/v1/cameras/nearby?lat=91&lon=0&radius_m=100")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn caps_nearby_radius() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/v1/cameras/nearby?lat=33&lon=-84&radius_m=5001")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn accepts_valid_report_without_verifying_it() {
        let body = serde_json::json!({
            "latitude": 33.0,
            "longitude": -84.0,
            "bearing_degrees": 90,
            "claimed_function": "unknown"
        });

        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/reports")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["status"], "accepted_unverified");
    }
}
