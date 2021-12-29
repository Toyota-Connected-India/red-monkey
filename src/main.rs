#![feature(proc_macro_hygiene, decl_macro)]

use env_logger::Env;
use log::{debug, error, info};
use std::process;
use tokio::join;
use tokio::net::TcpListener;

#[macro_use]
extern crate serde_derive;

mod config;
mod fault_config_server;
mod proxy;
mod store;

fn init_logger() {
    let env = Env::default()
        .filter_or("LOG_LEVEL", "debug")
        .write_style_or("LOG_STYLE", "always");
    env_logger::init_from_env(env);
}

#[tokio::main]
async fn main() {
    init_logger();

    let config = config::get_config().unwrap_or_else(|e| {
        error!("Failed to parse environment variables : {}", e);
        process::exit(0);
    });
    debug!("env configs: {:?}", config);

    let fault_store = store::mem_store::MemStore::new_db();

    let proxy = proxy::connection::Connection::new(
        config.redis_address.clone(),
        proxy::faulter::Faulter::new(fault_store.clone()),
    );

    let fault_config_server_future = tokio::spawn(async move {
        debug!("Starting fault config server");

        // run the fault config server
        fault_config_server::routes::run(fault_store).await.unwrap();
    });

    info!("Listening on port: {}", config.proxy_port);
    let listener = TcpListener::bind(&config.proxy_port).await.unwrap();

    let proxy_future = tokio::spawn(async move {
        loop {
            debug!("request received");
            let (socket, _addr) = listener.accept().await.unwrap();
            proxy.clone().handle_connection(socket).await;
        }
    });

    let _ = join!(fault_config_server_future, proxy_future);
}
