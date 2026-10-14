use anyhow::Result;
use serde::{Deserialize, Serialize};
use veilid_core::{DHTSchema, RecordKey, RouteId, RoutingContext, CRYPTO_KIND_VLD0};

use crate::{
    board::{
        board_keypair, cell_for, ensure_board, open_board, scan_cells, time_bucket, BOARD_SLOTS,
    },
    pedestrian::{publish_pointer, read_pointers, Pointer},
};

const SHARDS: u8 = 1;
const POINTER_TTL_SECS: u64 = 10 * 60;

#[derive(Serialize, Deserialize)]
pub struct RideRequest {
    pub cell: u64, // H3 cell, coarse on purpose
    pub expires_at: u64,
    pub max_price_cents: u32,
}

#[derive(Debug)]
pub struct PublishedRequest {
    pub request_key: RecordKey,
    pub route_id: RouteId,
    pub board_key: RecordKey,
    pub slot: u16,
}

pub async fn publish_request(
    rc: &RoutingContext,
    lat: f64,
    lng: f64,
    max_price_cents: u32,
) -> Result<PublishedRequest> {
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
    let details = RideRequest {
        cell: u64::from(cell),
        expires_at,
        max_price_cents,
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
        request_key: request_key.to_string(), // full key, parsed back by the driver
        expires_at,
    };
    let slot = publish_pointer(rc, &board_key, BOARD_SLOTS, &ptr, now).await?;
    rc.close_dht_record(board_key.clone()).await?;

    Ok(PublishedRequest {
        request_key,
        route_id: route.route_id,
        board_key,
        slot,
    })
}

pub async fn find_requests(rc: &RoutingContext, lat: f64, lng: f64) -> Result<()> {
    let driver_cell = cell_for(lat, lng)?;
    let neighbour_cells = scan_cells(driver_cell);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut pointers: Vec<Pointer> = vec![];

    for cell in neighbour_cells.iter() {
        println!("scanning cell # {:?}", cell);
        for shard in 0..SHARDS {
            let kp = board_keypair(*cell, time_bucket(now), shard)?;
            println!("key {:?}", kp.key());
            if let Ok(dht_key) = open_board(rc, &kp).await {
                let pts = read_pointers(rc, &dht_key.key(), BOARD_SLOTS).await;
                pointers = [pointers, pts].concat();
            }
        }
    }

    println!("finished");

    for p in pointers.iter() {
        println!("poiinter : {:?}", p);
    }

    Ok(())
}
