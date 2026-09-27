use roadwatch_api::app;

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("failed to bind RoadWatch API");

    println!("RoadWatch API listening on http://127.0.0.1:8080");
    axum::serve(listener, app())
        .await
        .expect("RoadWatch API server failed");
}
