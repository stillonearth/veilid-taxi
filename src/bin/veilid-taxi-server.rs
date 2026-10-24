use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        ConnectInfo,
    },
    response::IntoResponse,
    routing::any,
    Router,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json,
};
use axum_extra::{headers, TypedHeader};

use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use veilid_core::RoutingContext;

use veilid_taxi::{
    app::{P2PApp, P2PAppState},
    driver::{accept_pedestrian_ride_request, find_requests},
    pedestrian::{publish_request, PedestianRideRequest},
    veilid::app_config,
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
struct WebAppState {
    rc: RoutingContext,
    pr: Arc<Mutex<PedestrianRequests>>,
    p2p_app_state: P2PAppState,
}

type ApiError = (StatusCode, String);

fn internal(e: anyhow::Error) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// POST /requests: publish a ride request as a pedestrian
async fn create_request(
    State(state): State<WebAppState>,
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
    State(state): State<WebAppState>,
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
    State(state): State<WebAppState>,
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

// ---------------
// WebSocket Logic
// ---------------

async fn ws_handler(
    State(state): State<WebAppState>,
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state.p2p_app_state))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(mut socket: WebSocket, who: SocketAddr, p2p_app_state: P2PAppState) {
    loop {
        tokio::select! {
            Ok(ride_request) = p2p_app_state.on_driver_ride_request.recv_async() => {
                let json = serde_json::to_string(&ride_request).unwrap();
                socket
                    .send(axum::extract::ws::Message::Text(json.into()))
                    .await.unwrap();
            }
            Ok(_data) = p2p_app_state.on_driver_ride_response.recv_async() => {

            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = app_config("veilid-taxi-server");

    let mut app = P2PApp::new(config).await?;
    let app_state = app.app_state.clone();

    let _ = app
        .start(|veilid_api| async move {
            let app_state = WebAppState {
                rc: veilid_api.clone().routing_context()?,
                pr: Arc::new(Mutex::new(PedestrianRequests::new())),
                p2p_app_state: app_state,
            };

            serve(app_state, &args.listen).await?;
            Ok(())
        })
        .await;

    Ok(())
}

async fn serve(state: WebAppState, addr: &str) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/requests", get(list_requests).post(create_request))
        .route("/requests/accept", post(accept_ride_request))
        .route("/ws", any(ws_handler))
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
