use roadwatch_api::{app_with_state, AppState};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").ok();
    let pool = match database_url {
        Some(url) => Some(
            PgPoolOptions::new()
                .max_connections(10)
                .connect(&url)
                .await
                .expect("failed to connect to RoadWatch database"),
        ),
        None => {
            eprintln!("WARNING: DATABASE_URL not set; nearby queries return no records");
            None
        }
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("failed to bind RoadWatch API");

    println!("RoadWatch API listening on http://127.0.0.1:8080");
    axum::serve(listener, app_with_state(AppState { pool }))
        .await
        .expect("RoadWatch API server failed");
}
