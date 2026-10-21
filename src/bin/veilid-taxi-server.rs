use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use veilid_core::RoutingContext;

use veilid_taxi::{
    pedestrian::Pointer,
    ride_request::{accept_request, find_requests, publish_request, RideRequest},
    veilid::{app_config, update_callback, veilid_api_scope},
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:3000")]
    listen: String,
}

const DEFAULT_MAX_PRICE_CENTS: u32 = 100;

#[derive(Deserialize)]
struct CreateRequest {
    latitude: f64,
    longitude: f64,
    max_price_cents: Option<u32>,
}

#[derive(Serialize)]
struct CreatedRequest {
    request_key: String,
    route_id: String,
    board_key: String,
    slot: u16,
}

#[derive(Deserialize)]
struct AcceptRideRequest {
    route_blob: Vec<u8>,
    message: String,
}

#[derive(Deserialize)]
struct Location {
    latitude: f64,
    longitude: f64,
}

type ApiError = (StatusCode, String);

fn internal(e: anyhow::Error) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// POST /requests: publish a ride request as a pedestrian
async fn create_request(
    State(rc): State<RoutingContext>,
    Json(body): Json<CreateRequest>,
) -> Result<Json<CreatedRequest>, ApiError> {
    let published = publish_request(
        &rc,
        body.latitude,
        body.longitude,
        body.max_price_cents.unwrap_or(DEFAULT_MAX_PRICE_CENTS),
    )
    .await
    .map_err(internal)?;

    Ok(Json(CreatedRequest {
        request_key: published.request_key.to_string(),
        route_id: published.route_id.to_string(),
        board_key: published.board_key.to_string(),
        slot: published.slot,
    }))
}

/// POST /requests/accept: accept a ride request as a driver and message the pedestrian
async fn accept_ride_request(
    State(rc): State<RoutingContext>,
    Json(body): Json<AcceptRideRequest>,
) -> Result<StatusCode, ApiError> {
    accept_request(&rc, body.route_blob, body.message)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /requests?latitude=..&longitude=..: list ride requests near a location
async fn list_requests(
    State(rc): State<RoutingContext>,
    Query(loc): Query<Location>,
) -> Result<Json<Vec<(RideRequest, Vec<u8>)>>, ApiError> {
    let pointers = find_requests(&rc, loc.latitude, loc.longitude)
        .await
        .map_err(internal)?;
    Ok(Json(pointers))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = app_config("veilid-taxi-server");

    let _ = veilid_api_scope(update_callback, config, |veilid_api| async move {
        let rc = veilid_api.routing_context()?;
        serve(rc, &args.listen).await?;
        Ok(())
    })
    .await;

    Ok(())
}

async fn serve(rc: RoutingContext, addr: &str) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/requests", get(list_requests).post(create_request))
        .route("/requests/accept", post(accept_ride_request))
        .with_state(rc);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("REST API listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
