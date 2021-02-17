use env_logger::Env;
use futures::FutureExt;
use log::{debug, error, info};
use std::error::Error;
use std::process;
use tokio::net::TcpListener;

#[macro_use]
extern crate serde_derive;
mod config;
mod proxy;
mod store;

fn init_logger() {
    let env = Env::default()
        .filter_or("LOG_LEVEL", "debug")
        .write_style_or("LOG_STYLE", "always");
    env_logger::init_from_env(env);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_logger();

    let config = config::get_config().unwrap_or_else(|e| {
        error!("Failed to parse environment variables : {}", e);
        process::exit(0);
    });
    debug!("env configs: {:?}", config);

    let listener = TcpListener::bind(&config.proxy_port).await?;
    info!("Listening on port: {}", config.proxy_port);

    while let Ok((inbound, _)) = listener.accept().await {
        debug!("connection established!");

        let transfer = proxy::connection::handle_connection(inbound, config.redis_address.clone())
            .map(|r| {
                if let Err(e) = r {
                    error!("Error handling connection: {}", e);
                }
            });

        tokio::spawn(transfer);
    }

    let _fault_store = crate::store::mem_store::new();

    Ok(())
}
