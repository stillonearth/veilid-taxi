use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use veilid_core::{RecordKey, RoutingContext, Target, ValueSubkey};

use crate::{
    app::P2PTaxiMessage,
    board::{board_keypair, cell_for, open_board, scan_cells, time_bucket, BOARD_SLOTS, SHARDS},
    pedestrian::{deserealize_pointer, read_ride_request, PedestianRideRequest, Pointer},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriverRideRequest {
    pub pedestrian_ride_request: PedestianRideRequest,
    pub message: String,
    pub driver_route: Vec<u8>,
}

pub async fn find_requests(
    rc: &RoutingContext,
    lat: f64,
    lng: f64,
) -> Result<Vec<(PedestianRideRequest)>> {
    let driver_cell = cell_for(lat, lng)?;
    let neighbour_cells = scan_cells(driver_cell);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut pointers: Vec<Pointer> = vec![];
    let mut ride_requests: Vec<PedestianRideRequest> = vec![];

    for cell in neighbour_cells.iter() {
        for shard in 0..SHARDS {
            let kp = board_keypair(*cell, time_bucket(now), shard)?;
            if let Ok(dht_key) = open_board(rc, &kp).await {
                let pts = read_pointers(rc, &dht_key.key(), BOARD_SLOTS).await;
                pointers = [pointers, pts].concat();
            }
        }
    }

    for pointer in pointers.iter() {
        let request_key = RecordKey::from(pointer.request_key.clone());
        if let Ok((mut ride_request, route_blob)) =
            read_request_from_dht(rc, request_key.clone()).await
        {
            ride_request.route_blob = route_blob.clone();
            ride_request.request_key = request_key.clone().to_string();
            ride_requests.push(ride_request);
        }
    }

    Ok(ride_requests)
}

pub async fn read_pointers(rc: &RoutingContext, board: &RecordKey, slots: u16) -> Vec<Pointer> {
    let mut pointers: Vec<Pointer> = vec![];

    for slot in 0..slots {
        if let Ok(cur) = rc
            .get_dht_value(board.clone(), ValueSubkey::from(slot), true)
            .await
        {
            let data = cur.as_ref().map(|v| v.data());
            if let Some(pointer) = deserealize_pointer(data) {
                pointers.push(pointer);
            }
        }
    }
    pointers
}

pub async fn read_request_from_dht(
    rc: &RoutingContext,
    request_key: RecordKey,
) -> Result<(PedestianRideRequest, Vec<u8>)> {
    let _ = rc.open_dht_record(request_key.clone(), None).await?;
    let result = read_request_values(rc, &request_key).await;
    let _ = rc.close_dht_record(request_key).await;
    result
}

async fn read_request_values(
    rc: &RoutingContext,
    request_key: &RecordKey,
) -> Result<(PedestianRideRequest, Vec<u8>)> {
    // force_refresh = true: a driver node has no local copy of the pedestrian's record
    let details = rc
        .get_dht_value(request_key.clone(), ValueSubkey::from(0u16), true)
        .await?;
    let ride_request = read_ride_request(details.as_ref().map(|v| v.data()))
        .ok_or_else(|| anyhow!("request details missing or malformed"))?;

    let route = rc
        .get_dht_value(request_key.clone(), ValueSubkey::from(1u16), true)
        .await?
        .ok_or_else(|| anyhow!("request route blob missing"))?;

    Ok((ride_request, route.data().to_vec()))
}

/// Accept a ride request as a driver: send `message` to the pedestrian's private route.
pub async fn accept_pedestrian_ride_request(
    rc: &RoutingContext,
    route_blob: Vec<u8>,
    pedestrian_ride_request: PedestianRideRequest,
    message: String,
) -> Result<DriverRideRequest> {
    let api = rc.api();
    let pedestrian_route_id = api.import_remote_private_route(route_blob)?;
    let driver_route = api.new_private_route().await?;

    let driver_ride_request = DriverRideRequest {
        message: message.clone(),
        driver_route: driver_route.blob,
        pedestrian_ride_request,
    };

    let message = P2PTaxiMessage::DriverRideRequest(driver_ride_request.clone());
    let serialized_message = serde_json::to_vec(&message)?;

    rc.app_message(Target::RouteId(pedestrian_route_id), serialized_message)
        .await?;

    Ok(driver_ride_request)
}
