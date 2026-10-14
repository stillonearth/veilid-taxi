use anyhow::Result;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use veilid_core::{RecordKey, RoutingContext, ValueSubkey};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Pointer {
    pub request_key: String, // key of the pedestrian's request record
    pub expires_at: u64,     // unix secs
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
        println!("slot {}", slot);
        if let Ok(cur) = rc
            .get_dht_value(board.clone(), ValueSubkey::from(slot), true)
            .await
        {
            println!("record read");
            let data = cur.as_ref().map(|v| v.data());
            if let Some(pointer) = read_pointer(data) {
                pointers.push(pointer);
            }
        }
    }
    pointers
}
