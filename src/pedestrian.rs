use std::default;

use anyhow::Result;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use veilid_core::{
    DHTSchema, RecordKey, RouteId, RoutingContext, Target, ValueSubkey, CRYPTO_KIND_VLD0,
};

use crate::{
    app::P2PTaxiMessage,
    board::{
        board_keypair, cell_for, ensure_board, time_bucket, BOARD_SLOTS, POINTER_TTL_SECS, SHARDS,
    },
};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PedestianRideRequest {
    #[serde(skip_deserializing)]
    pub request_key: String,
    #[serde(skip_deserializing)]
    pub route_blob: Vec<u8>,

    pub cell: u64,
    pub expires_at: u64,
    pub max_price_cents: u32,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct PublishedPedestrianRequest {
    pub request_key: RecordKey,
    pub route_id: RouteId,
    pub board_key: RecordKey,
    pub slot: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Pointer {
    pub request_key: RecordKey, // key of the pedestrian's request record
    pub expires_at: u64,        // unix secs
}

fn is_free(raw: Option<&[u8]>, now: u64) -> bool {
    match raw.and_then(|b| serde_json::from_slice::<Pointer>(b).ok()) {
        Some(p) => p.expires_at <= now, // expired pointers can be overwritten
        None => true,                   // empty or garbage
    }
}

pub fn deserealize_pointer(raw: Option<&[u8]>) -> Option<Pointer> {
    raw.and_then(|b| serde_json::from_slice::<Pointer>(b).ok())
}

pub fn read_ride_request(raw: Option<&[u8]>) -> Option<PedestianRideRequest> {
    raw.and_then(|b| serde_json::from_slice::<PedestianRideRequest>(b).ok())
}

pub async fn publish_pointer(
    rc: &RoutingContext,
    board: &RecordKey,
    slots: u16,
    ptr: &Pointer,
    now: u64,
) -> anyhow::Result<u16> {
    let bytes = serde_json::to_vec(ptr)?;
    let mut order: Vec<u16> = (0..slots).collect();
    order.shuffle(&mut rand::rng());

    for slot in order {
        // force_refresh = true: ask the network, not just the local cache
        let cur = rc
            .get_dht_value(board.clone(), ValueSubkey::from(slot), true)
            .await?;
        if !is_free(cur.as_ref().map(|v| v.data()), now) {
            continue;
        }

        // Ok(None) = accepted, Ok(Some(_)) = someone wrote a newer value first
        if rc
            .set_dht_value(board.clone(), ValueSubkey::from(slot), bytes.clone(), None)
            .await?
            .is_some()
        {
            continue; // lost the race, try the next slot
        }

        // Verify that our write is the one that stuck
        let back = rc
            .get_dht_value(board.clone(), ValueSubkey::from(slot), true)
            .await?;
        if back.as_ref().map(|v| v.data()) == Some(bytes.as_slice()) {
            return Ok(slot);
        }
    }
    anyhow::bail!("no free slot on board")
}

pub async fn publish_request(
    rc: &RoutingContext,
    lat: f64,
    lng: f64,
    max_price_cents: u32,
) -> Result<PublishedPedestrianRequest> {
    let api = rc.api();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let expires_at = now + POINTER_TTL_SECS;

    // 1. Private route: where drivers send their offers
    let route = api.new_private_route().await?;

    // 2. Request record. Owner None: Veilid generates a throwaway owner keypair,
    //    and the returned open record is writable by it.
    let desc = rc
        .create_dht_record(CRYPTO_KIND_VLD0, DHTSchema::dflt(2)?, None)
        .await?;
    let request_key = desc.key(); // verify accessor name in your version

    let cell = cell_for(lat, lng)?;
    let details = PedestianRideRequest {
        cell: u64::from(cell),
        expires_at,
        max_price_cents,
        request_key: request_key.to_string(),
        route_blob: vec![],
    };
    rc.set_dht_value(request_key.clone(), 0, serde_json::to_vec(&details)?, None)
        .await?;
    rc.set_dht_value(request_key.clone(), 1, route.blob.clone(), None)
        .await?;

    // 3. Board pointer, written only after the record is filled in
    let shard = rand::random::<u8>() % SHARDS;
    let kp = board_keypair(cell, time_bucket(now), shard)?;
    let board_key = ensure_board(rc, &kp).await?.key();

    let ptr = Pointer {
        request_key: request_key.clone(),
        expires_at,
    };
    let slot = publish_pointer(rc, &board_key, BOARD_SLOTS, &ptr, now).await?;
    rc.close_dht_record(board_key.clone()).await?;

    Ok(PublishedPedestrianRequest {
        request_key: request_key.clone(),
        route_id: route.route_id,
        board_key,
        slot,
    })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PedestrianDecision {
    Yes,
    No,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriverRideResponse {
    pub pedestrian_ride_request: PedestianRideRequest,
    pub decision: PedestrianDecision,
    pub driver_route: Vec<u8>,
}

pub async fn respond_to_driver_ride_request(
    rc: &RoutingContext,
    driver_route: Vec<u8>,
    pedestrian_ride_request: PedestianRideRequest,
    decision: PedestrianDecision,
) -> Result<DriverRideResponse> {
    let api = rc.api();
    let driver_route_id = api.import_remote_private_route(driver_route.clone())?;

    let driver_ride_response = DriverRideResponse {
        decision,
        driver_route: driver_route.clone(),
        pedestrian_ride_request,
    };
    let message = P2PTaxiMessage::DriverRideResponse(driver_ride_response.clone());
    let serialized_message = serde_json::to_vec(&message)?;

    rc.app_message(Target::RouteId(driver_route_id), serialized_message)
        .await?;

    Ok(driver_ride_response)
}
