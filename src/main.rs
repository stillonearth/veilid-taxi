mod board;
mod pedestrian;
mod ride_request;
mod veilid;

use anyhow::{Error, Result};
use clap::Parser;

use veilid_core::{VeilidConfig, VeilidConfigProtectedStore, VeilidConfigTableStore};

use crate::{
    ride_request::find_requests,
    veilid::{update_callback, veilid_api_scope},
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    pedestrian: bool,

    #[arg(long)]
    driver: bool,

    #[arg(long)]
    longitude: f64,

    #[arg(long)]
    latitude: f64,
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
        namespace: "veilid-taxi".into(),

        protected_store: VeilidConfigProtectedStore {
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

    if !(args.pedestrian || args.driver) {
        return Ok(());
    }

    let _ = veilid_api_scope(update_callback, config, |veilid_api| async move {
        let rc = veilid_api.routing_context()?;

        if args.pedestrian {
            let published_request =
                ride_request::publish_request(&rc, args.latitude, args.longitude, 100).await?;

            println!("request published: {:?}", published_request);
        }

        if args.driver {
            println!("finding pedestrian requests");
            find_requests(&rc, args.latitude, args.longitude).await?;
        }

        tokio::signal::ctrl_c().await?;

        Ok(())
    })
    .await;

    Ok(())
}
