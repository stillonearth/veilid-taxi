mod board;
mod pedestrian;

use anyhow::{Error, Result};
use clap::Parser;
use rand;
use std::time::Duration;
use std::{future::Future, sync::Arc};
use tokio::time::sleep;
use tracing::info;
use veilid_core::AttachmentState::*;
use veilid_core::VeilidUpdate::Network;
use veilid_core::{
    api_startup, VeilidAPI, VeilidConfig, VeilidConfigProtectedStore, VeilidConfigTableStore,
    VeilidUpdate,
};

use crate::board::BOARD_SLOTS;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    pedestrian: bool,

    #[arg(long)]
    longitude: f32,

    #[arg(long)]
    latitude: f32,
}

async fn wait_for_attached(api: &VeilidAPI) -> Result<()> {
    info!("Awaiting attachment");
    loop {
        let state = api.get_state().await?;
        match state.attachment.state {
            AttachedWeak | AttachedFair | AttachedGood | AttachedStrong | AttachedFull => {
                info!("Awaiting attachment, done");
                return Ok(());
            }
            _ => (),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) async fn wait_for_network_start(api: &VeilidAPI) -> Result<(), Error> {
    info!("awaiting network initialization");
    loop {
        let vs = api.get_state().await?;
        if vs.network.started && !vs.network.peers.is_empty() {
            info!(
                "Awaiting network initialization, done ({} peer(s))",
                vs.network.peers.len()
            );
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) async fn wait_for_public_internet_ready(api: &VeilidAPI) -> Result<(), Error> {
    info!("Awaiting 'public_internet_ready'");
    loop {
        let state = api.get_state().await?;
        if state.attachment.public_internet_ready {
            break;
        }
        sleep(Duration::from_secs(5)).await;
    }

    info!("Awaiting 'public_internet_ready', done");
    Ok(())
}

async fn veilid_api_scope<'a, F: Future<Output = Result<T, Box<dyn std::error::Error>>>, T>(
    update_callback: impl Fn(VeilidUpdate) + Send + Sync + 'static,
    veilid_config: VeilidConfig,
    scope: impl FnOnce(VeilidAPI) -> F + Send + Sync + 'a,
) -> Result<T, Box<dyn std::error::Error>> {
    // Startup Veilid node
    // Note: future is boxed due to its size and our aggressive clippy lints
    let veilid_api = Box::pin(api_startup(Arc::new(update_callback), veilid_config)).await?;

    // Attach to the network
    veilid_api.attach().await?;

    wait_for_public_internet_ready(&veilid_api).await?;

    wait_for_network_start(&veilid_api).await?;
    println!("veilid network started");
    wait_for_attached(&veilid_api).await?;
    println!("veilid attached");

    // Operate the Veilid node inside a scope
    let res = scope(veilid_api.clone()).await;

    // Clean shutdown
    veilid_api.shutdown().await;

    // Return result
    res
}

fn update_callback(update: VeilidUpdate) {
    match update {
        VeilidUpdate::Log(_veilid_log) => {}
        VeilidUpdate::AppMessage(veilid_app_message) => {
            let msg = String::from_utf8_lossy(veilid_app_message.message());
            println!("AppMessage received: {msg}");
        }
        VeilidUpdate::AppCall(_veilid_app_call) => {}
        VeilidUpdate::Attachment(_veilid_state_attachment) => {}
        Network(msg) => {
            println!(
                "Network: Node Ids: {}, Peers {}, bytes/sec [{} up] [{} down]",
                if msg.node_ids.is_empty() {
                    "(none assigned)".to_string()
                } else {
                    msg.node_ids
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                },
                msg.peers.len(),
                msg.bps_up,
                msg.bps_down
            )
        }
        VeilidUpdate::Config(_veilid_state_config) => {}
        VeilidUpdate::RouteChange(veilid_route_change) => {
            // XXX: If this happens, the route is dead, and a new one should be generated and
            // exchanged. This will no longer be necessary after DHT Route Autopublish is implemented in veilid-core v0.6.0
            println!("{veilid_route_change:?}");
        }
        VeilidUpdate::ValueChange(_veilid_value_change) => {}
        VeilidUpdate::Shutdown => {}
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    // Set up a config for this application
    let exe_dir = std::env::current_exe()
        .map(|x| x.parent().map(|p| p.to_owned()))
        .ok()
        .flatten()
        .unwrap_or(".".into());
    let config = VeilidConfig {
        program_name: "Example Veilid".into(),
        namespace: "veilid-example".into(),

        protected_store: VeilidConfigProtectedStore {
            // IMPORTANT: don't do this in production
            // This avoids prompting for a password and is insecure
            always_use_insecure_storage: true,
            directory: exe_dir
                .join(".veilid/protected_store")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        },
        table_store: VeilidConfigTableStore {
            directory: exe_dir
                .join(".veilid/table_store")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    if args.pedestrian {
        println!("about to publish a request");

        let _ = veilid_api_scope(update_callback, config, |veilid_api| async move {
            let rc = veilid_api.routing_context()?;

            let cell = board::cell_for(50.1109, 8.6821)?; // Frankfurt
            let shard = rand::random::<u8>() % 4; // N shards against hot cells
            let kp = board::board_keypair(cell, board::now_bucket(), shard)?;
            let board = board::ensure_board(&rc, &kp).await?;

            // rc.set_dht_value(board.key(), subkey, data, options)

            let request_pointer = pedestrian::Pointer {
                request_key: "test".to_string(),
                expires_at: board::now_bucket() + 60 * 5,
            };

            pedestrian::publish_pointer(
                &rc,
                &board.key(),
                BOARD_SLOTS,
                &request_pointer,
                board::now_bucket(),
            )
            .await?;

            println!("pointer published");

            tokio::signal::ctrl_c().await?;

            Ok(())
        })
        .await;
    }

    Ok(())
}
