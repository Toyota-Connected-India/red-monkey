#![feature(proc_macro_hygiene, decl_macro)]

use env_logger::Env;
use futures::future::FutureExt;
use log::{debug, error, info};
use std::boxed::Box;
use std::error::Error;
use std::process;
use std::sync::Arc;
use tokio::net::TcpListener;

#[macro_use]
extern crate serde_derive;

mod config;
mod proxy;
mod server;
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

    let fault_store = store::mem_store::MemStore::new();

    let proxy = Arc::new(proxy::connection::Connection::new(
        Box::leak(config.redis_address.into_boxed_str()),
        proxy::faulter::Faulter::new(fault_store.clone_box()),
    ));

    // run the fault config server
    server::routes::run(fault_store);

    while let Ok((inbound, _)) = listener.accept().await {
        let proxy = proxy.clone();
        debug!("request received");

        let _transfer = proxy
            .handle_connection(inbound)
            .map(|r| {
                if let Err(e) = r {
                    error!("Error handling connection: {}", e);
                }
            })
            .await;
    }

    Ok(())
}
