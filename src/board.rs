use anyhow::Result;
use blake3::Hasher;
use ed25519_dalek::SigningKey;
use h3o::{CellIndex, LatLng, Resolution};
use veilid_core::{
    BareKeyPair, BarePublicKey, BareSecretKey, DHTRecordDescriptor, DHTSchema, KeyPair,
    RoutingContext, CRYPTO_KIND_VLD0,
};

/// Bump the version to rotate every board at once.
const DOMAIN: &str = "mytaxi/v1/board";
const BUCKET_SECS: u64 = 15 * 60;
const H3_RES: Resolution = Resolution::Eight;
pub const BOARD_SLOTS: u16 = 8;

pub fn cell_for(lat: f64, lng: f64) -> Result<CellIndex> {
    Ok(LatLng::new(lat, lng)?.to_cell(H3_RES))
}

pub fn time_bucket(unix_secs: u64) -> u64 {
    unix_secs / BUCKET_SECS
}

/// Same inputs give the same 32-byte seed on every device.
pub fn board_seed(cell: CellIndex, bucket: u64, shard: u8) -> [u8; 32] {
    let mut h = Hasher::new_derive_key(DOMAIN); // BLAKE3 KDF mode: built-in domain separation
    h.update(&u64::from(cell).to_be_bytes());
    h.update(&bucket.to_be_bytes());
    h.update(&[shard]);
    *h.finalize().as_bytes()
}

pub fn board_keypair(cell: CellIndex, bucket: u64, shard: u8) -> Result<KeyPair> {
    let seed = board_seed(cell, bucket, shard);
    let signing = SigningKey::from_bytes(&seed);
    let public = signing.verifying_key();

    // Constructor names inferred from veilid-core 0.5.x: verify for your version
    let kp = KeyPair::new(
        CRYPTO_KIND_VLD0,
        BareKeyPair::new(
            BarePublicKey::new(public.as_bytes()),
            BareSecretKey::new(&seed),
        ),
    );
    Ok(kp)
}

/// Cells to scan as a driver: own cell plus the ring of neighbours.
pub fn scan_cells(center: CellIndex) -> Vec<CellIndex> {
    center.grid_disk::<Vec<_>>(0)
}

/// Create the board if needed and open it with its deterministic, unencrypted key.
pub async fn ensure_board(rc: &RoutingContext, kp: &KeyPair) -> Result<DHTRecordDescriptor> {
    let schema = DHTSchema::dflt(BOARD_SLOTS)?;
    if let Ok(desc) = rc
        .create_dht_record(CRYPTO_KIND_VLD0, schema, Some(kp.clone()))
        .await
    {
        rc.close_dht_record(desc.key()).await?;
    }
    open_board(rc, kp).await
}

pub async fn open_board(rc: &RoutingContext, kp: &KeyPair) -> Result<DHTRecordDescriptor> {
    let schema = DHTSchema::dflt(BOARD_SLOTS)?;
    let key = rc.api().get_dht_record_key(schema, kp.key(), None).await?;
    Ok(rc.open_dht_record(key, Some(kp.clone())).await?)
}
