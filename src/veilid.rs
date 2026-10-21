use anyhow::{Error, Result};

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

/// Veilid config with storage next to the executable, under `.veilid/<name>`,
/// so different binaries don't share (and lock) the same stores.
pub fn app_config(name: &str) -> VeilidConfig {
    let exe_dir = std::env::current_exe()
        .map(|x| x.parent().map(|p| p.to_owned()))
        .ok()
        .flatten()
        .unwrap_or(".".into());
    let store_dir = exe_dir.join(".veilid").join(name);

    VeilidConfig {
        program_name: name.into(),
        namespace: "veilid-taxi".into(),

        protected_store: VeilidConfigProtectedStore {
            always_use_insecure_storage: true,
            directory: store_dir
                .join("protected_store")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        },
        table_store: VeilidConfigTableStore {
            directory: store_dir.join("table_store").to_string_lossy().to_string(),
            ..Default::default()
        },
        ..Default::default()
    }
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

async fn wait_for_network_start(api: &VeilidAPI) -> Result<(), Error> {
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

async fn wait_for_public_internet_ready(api: &VeilidAPI) -> Result<(), Error> {
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

pub async fn veilid_api_scope<'a, F: Future<Output = Result<T, Box<dyn std::error::Error>>>, T>(
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
    info!("veilid network started");
    wait_for_attached(&veilid_api).await?;
    info!("veilid attached");

    // Operate the Veilid node inside a scope
    let res = scope(veilid_api.clone()).await;

    // Clean shutdown
    veilid_api.shutdown().await;

    // Return result
    res
}

pub fn update_callback(update: VeilidUpdate) {
    match update {
        VeilidUpdate::Log(_veilid_log) => {}
        VeilidUpdate::AppMessage(veilid_app_message) => {
            let msg = String::from_utf8_lossy(veilid_app_message.message());
            // println, not info!: no tracing subscriber is installed, and the
            // pedestrian needs to see driver messages
            println!("AppMessage received: {msg}");
        }
        VeilidUpdate::AppCall(_veilid_app_call) => {}
        VeilidUpdate::Attachment(_veilid_state_attachment) => {}
        Network(msg) => {
            info!(
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
            info!("{veilid_route_change:?}");
        }
        VeilidUpdate::ValueChange(_veilid_value_change) => {}
        VeilidUpdate::Shutdown => {}
    }
}
