use anyhow::{Error, Result};
use clap::Parser;

use veilid_taxi::{
    ride_request::{find_requests, publish_request},
    veilid::{app_config, update_callback, veilid_api_scope},
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

    if !(args.pedestrian || args.driver) {
        return Ok(());
    }

    let config = app_config("veilid-taxi-cli");

    let _ = veilid_api_scope(update_callback, config, |veilid_api| async move {
        let rc = veilid_api.routing_context()?;

        if args.pedestrian {
            let published_request =
                publish_request(&rc, args.latitude, args.longitude, 100).await?;
        }

        if args.driver {
            println!("finding pedestrian requests");
            let pointers = find_requests(&rc, args.latitude, args.longitude).await?;

            println!("{} requests found", pointers.len());

            for (request, route) in pointers.iter() {
                println!("{:?}", request);
            }
        }

        tokio::signal::ctrl_c().await?;

        Ok(())
    })
    .await;

    Ok(())
}
