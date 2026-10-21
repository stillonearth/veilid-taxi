use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use veilid_core::RoutingContext;

use veilid_taxi::{
    driver::{accept_pedestrian_ride_request, find_requests},
    pedestrian::{publish_request, PedestianRideRequest},
    veilid::{app_config, update_callback, veilid_api_scope},
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:3000")]
    listen: String,
}

const DEFAULT_MAX_PRICE_CENTS: u32 = 100;

#[derive(Deserialize)]
struct CreateRideRequest {
    latitude: f64,
    longitude: f64,
    max_price_cents: Option<u32>,
}

#[derive(Serialize)]
struct CreatedRideRequest {
    request_key: String,
    route_id: String,
    board_key: String,
    slot: u16,
}

#[derive(Deserialize)]
struct AcceptRideRequest {
    request_key: String,
    message: String,
}

#[derive(Serialize)]
struct AcceptedRideRequest {
    request_key: String,
    driver_route: Vec<u8>,
    pedestrian_route: Vec<u8>,
}

#[derive(Deserialize)]
struct Location {
    latitude: f64,
    longitude: f64,
}

type PedestrianRequests = HashMap<String, PedestianRideRequest>;

#[derive(Clone)]
struct AppState {
    rc: RoutingContext,
    pr: Arc<Mutex<PedestrianRequests>>,
}

type ApiError = (StatusCode, String);

fn internal(e: anyhow::Error) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// POST /requests: publish a ride request as a pedestrian
async fn create_request(
    State(state): State<AppState>,
    Json(body): Json<CreateRideRequest>,
) -> Result<Json<CreatedRideRequest>, ApiError> {
    let published = publish_request(
        &state.rc,
        body.latitude,
        body.longitude,
        body.max_price_cents.unwrap_or(DEFAULT_MAX_PRICE_CENTS),
    )
    .await
    .map_err(internal)?;

    Ok(Json(CreatedRideRequest {
        request_key: published.request_key.to_string(),
        route_id: published.route_id.to_string(),
        board_key: published.board_key.to_string(),
        slot: published.slot,
    }))
}

/// POST /requests/accept: accept a ride request as a driver and message the pedestrian
async fn accept_ride_request(
    State(state): State<AppState>,
    Json(body): Json<AcceptRideRequest>,
) -> Result<Json<AcceptedRideRequest>, ApiError> {
    let pr = state.pr.lock().await;
    let pedesrtian_ride_request = pr
        .get(&body.request_key)
        .ok_or((StatusCode::NOT_FOUND, "unknown request_key".to_string()))?;

    let driver_ride_request = accept_pedestrian_ride_request(
        &state.rc,
        pedesrtian_ride_request.route_blob.clone(),
        pedesrtian_ride_request.clone(),
        body.message,
    )
    .await
    .map_err(internal)?;

    Ok(Json(AcceptedRideRequest {
        request_key: pedesrtian_ride_request.request_key.clone(),
        pedestrian_route: pedesrtian_ride_request.route_blob.clone(),
        driver_route: driver_ride_request.driver_route.clone(),
    }))
}

/// GET /requests?latitude=..&longitude=..: list ride requests near a location
async fn list_requests(
    State(state): State<AppState>,
    Query(loc): Query<Location>,
) -> Result<Json<Vec<PedestianRideRequest>>, ApiError> {
    let requests = find_requests(&state.rc, loc.latitude, loc.longitude)
        .await
        .map_err(internal)?;

    let mut pr = state.pr.lock().await;
    for request in requests.iter() {
        pr.insert(request.request_key.clone(), request.clone());
    }

    Ok(Json(requests))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = app_config("veilid-taxi-server");

    let _ = veilid_api_scope(update_callback, config, |veilid_api| async move {
        let app_state = AppState {
            rc: veilid_api.routing_context()?,
            pr: Arc::new(Mutex::new(PedestrianRequests::new())),
        };

        serve(app_state, &args.listen).await?;
        Ok(())
    })
    .await;

    Ok(())
}

async fn serve(state: AppState, addr: &str) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/requests", get(list_requests).post(create_request))
        .route("/requests/accept", post(accept_ride_request))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("REST API listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
