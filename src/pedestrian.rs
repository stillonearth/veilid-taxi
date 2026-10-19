use anyhow::{anyhow, Result};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use veilid_core::{RecordKey, RoutingContext, ValueSubkey};

use crate::ride_request::RideRequest;

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

fn read_pointer(raw: Option<&[u8]>) -> Option<Pointer> {
    raw.and_then(|b| serde_json::from_slice::<Pointer>(b).ok())
}

fn read_ride_request(raw: Option<&[u8]>) -> Option<RideRequest> {
    raw.and_then(|b| serde_json::from_slice::<RideRequest>(b).ok())
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

pub async fn read_pointers(rc: &RoutingContext, board: &RecordKey, slots: u16) -> Vec<Pointer> {
    let mut pointers: Vec<Pointer> = vec![];

    for slot in 0..slots {
        if let Ok(cur) = rc
            .get_dht_value(board.clone(), ValueSubkey::from(slot), true)
            .await
        {
            let data = cur.as_ref().map(|v| v.data());
            if let Some(pointer) = read_pointer(data) {
                pointers.push(pointer);
            }
        }
    }
    pointers
}

pub async fn read_request(
    rc: &RoutingContext,
    request_key: RecordKey,
) -> Result<(RideRequest, Vec<u8>)> {
    let mut ride_request: Option<RideRequest> = None;
    let mut route_blob: Option<Vec<u8>> = None;

    if let Ok(details_data) = rc
        .get_dht_value(request_key.clone(), ValueSubkey::from(0 as u16), true)
        .await
    {
        let data = details_data.as_ref().map(|v| v.data());
        ride_request = read_ride_request(data);
    }

    if let Ok(details_data) = rc
        .get_dht_value(request_key.clone(), ValueSubkey::from(1 as u16), true)
        .await
    {
        let data = details_data.as_ref().map(|v| v.data());
        if data.is_some() {
            route_blob = Some(data.unwrap().into());
        }
    }

    if ride_request.is_some() && route_blob.is_some() {
        let ride_request = ride_request.unwrap();
        let route_blob = route_blob.unwrap();

        return Ok((ride_request, route_blob));
    }

    Err(anyhow!("Operation failed"))
}
