use anyhow::{Error, Result};
use clap::Parser;

use veilid_taxi::{
    app::P2PApp,
    driver::{accept_pedestrian_ride_request, find_requests},
    pedestrian::publish_request,
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

    /// As a driver, accept the found requests and message each pedestrian
    #[arg(long, requires = "driver")]
    accept: bool,

    /// Message sent to the pedestrian when accepting
    #[arg(long, default_value = "hello")]
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    if !(args.pedestrian || args.driver) {
        return Ok(());
    }

    let config = app_config("veilid-taxi-cli");

    // let app = P2PApp {};

    let _ = veilid_api_scope(update_callback, config, |veilid_api| async move {
        let rc = veilid_api.routing_context()?;

        if args.pedestrian {
            let published_request = publish_request(&rc, args.latitude, args.longitude, 100)
                .await
                .unwrap();
            println!("request published: {:?}", published_request);
            println!("waiting for drivers, ctrl-c to quit");
        }

        if args.driver {
            println!("finding pedestrian requests");
            let pointers = find_requests(&rc, args.latitude, args.longitude).await?;

            println!("{} requests found", pointers.len());

            for (i, request) in pointers.iter().enumerate() {
                println!("#{} {:?}", i + 1, request);

                // if args.accept {
                //     match accept_request(&rc, route.clone(), args.message.clone()).await {
                //         Ok(()) => println!("#{} accepted, sent {:?}", i + 1, args.message),
                //         Err(e) => println!("#{} accept failed: {e}", i + 1),
                //     }
                // }
            }
        }

        tokio::signal::ctrl_c().await?;

        Ok(())
    })
    .await;

    Ok(())
}
