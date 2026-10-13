mod board;
mod pedestrian;
mod ride_request;
mod veilid;

use anyhow::{Error, Result};
use clap::Parser;
use rand;

use veilid_core::{VeilidConfig, VeilidConfigProtectedStore, VeilidConfigTableStore, VeilidUpdate};

use crate::{
    board::BOARD_SLOTS,
    veilid::{update_callback, veilid_api_scope},
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    pedestrian: bool,

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

            let published_request =
                ride_request::publish_request(&rc, args.latitude, args.longitude, 100)
                    .await
                    .unwrap();

            println!("request published: {:?}", published_request);

            tokio::signal::ctrl_c().await?;

            Ok(())
        })
        .await;
    }

    Ok(())
}
