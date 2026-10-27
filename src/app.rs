use anyhow::Result;
use flume::{unbounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::{future::Future, sync::Arc};
use tracing::info;
use veilid_core::tools::spawn;
use veilid_core::VeilidUpdate::Network;
use veilid_core::{api_startup, VeilidAPI, VeilidConfig, VeilidUpdate};

use crate::driver::DriverRideRequest;
use crate::pedestrian::DriverRideResponse;
use crate::veilid::{wait_for_attached, wait_for_network_start, wait_for_public_internet_ready};

#[derive(Serialize, Deserialize)]
pub enum P2PTaxiMessage {
    DriverRideRequest(DriverRideRequest),
    DriverRideResponse(DriverRideResponse),
}

#[derive(Clone)]
pub struct P2PAppState {
    pub on_driver_ride_request: Receiver<DriverRideRequest>,
    send_driver_ride_request: Sender<DriverRideRequest>,
    pub on_driver_ride_response: Receiver<DriverRideResponse>,
    send_driver_ride_response: Sender<DriverRideResponse>,
}

impl P2PAppState {
    pub fn new() -> Self {
        let (send_driver_ride_request, on_driver_ride_request): (
            Sender<DriverRideRequest>,
            Receiver<DriverRideRequest>,
        ) = unbounded();

        let (send_driver_ride_response, on_driver_ride_response): (
            Sender<DriverRideResponse>,
            Receiver<DriverRideResponse>,
        ) = unbounded();

        P2PAppState {
            send_driver_ride_request,
            on_driver_ride_request,
            send_driver_ride_response,
            on_driver_ride_response,
        }
    }
}

#[derive(Clone)]
pub struct P2PApp {
    veilid_api: VeilidAPI,
    on_veilid_update: Receiver<VeilidUpdate>,
    pub app_state: P2PAppState,
}

impl P2PApp {
    pub async fn new(veilid_config: VeilidConfig) -> Result<Self> {
        let (veilid_update_sender, veilid_update_receiver): (
            Sender<veilid_core::VeilidUpdate>,
            Receiver<veilid_core::VeilidUpdate>,
        ) = unbounded();

        let update_callback = Arc::new(move |change: veilid_core::VeilidUpdate| {
            if let Err(e) = veilid_update_sender.send(change.clone()) {
                let change = e.into_inner();
                info!("error sending veilid update callback: {:?}", change);
            }

            match change {
                VeilidUpdate::Log(_veilid_log) => {}
                VeilidUpdate::AppMessage(_veilid_app_message) => {}
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
        });

        let veilid_api = Box::pin(api_startup(update_callback, veilid_config)).await?;
        return Ok(Self {
            on_veilid_update: veilid_update_receiver,
            veilid_api: veilid_api,
            app_state: P2PAppState::new(),
        });
    }

    pub async fn network_loop(&mut self) -> Result<()> {
        while self.network_loop_cycle().await? {}
        Ok(())
    }

    /// Handles one update; returns false once veilid has shut down.
    pub async fn network_loop_cycle(&mut self) -> Result<bool> {
        let res = self.on_veilid_update.recv_async().await?;

        match res {
            VeilidUpdate::Shutdown => return Ok(false),
            VeilidUpdate::AppMessage(veilid_app_message) => {
                if let Ok(p2p_taxi_app_message) =
                    serde_json::from_slice::<P2PTaxiMessage>(veilid_app_message.message())
                {
                    match p2p_taxi_app_message {
                        P2PTaxiMessage::DriverRideRequest(driver_ride_request) => {
                            let _ = self
                                .app_state
                                .send_driver_ride_request
                                .send_async(driver_ride_request)
                                .await;
                        }
                        P2PTaxiMessage::DriverRideResponse(driver_ride_response) => {
                            let _ = self
                                .app_state
                                .send_driver_ride_response
                                .send_async(driver_ride_response)
                                .await;
                        }
                    }
                }
            }
            _ => (),
        };

        Ok(true)
    }

    pub async fn start<
        'a,
        F: Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
        T,
    >(
        &mut self,
        scope: impl FnOnce(VeilidAPI) -> F + Sync + 'a,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        println!("attempt to spawn");

        let mut app = self.clone();

        let handle = spawn("network_loop", async move { app.network_loop().await });

        // Attach to the network
        self.veilid_api.attach().await?;

        wait_for_public_internet_ready(&self.veilid_api).await?;

        wait_for_network_start(&self.veilid_api).await?;
        info!("veilid network started");
        wait_for_attached(&self.veilid_api).await?;
        info!("veilid attached");

        // Operate the Veilid node inside a scope
        let res = scope(self.veilid_api.clone()).await;
        handle.abort().await;

        // Clean shutdown
        self.veilid_api.clone().shutdown().await;

        // Return result
        res
    }
}
