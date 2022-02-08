#![feature(proc_macro_hygiene, decl_macro)]

use env_logger::Env;
use log::{debug, error, info};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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
async fn main() -> Result<(), anyhow::Error> {
    init_logger();
    let config = config::get_config().expect("Error reading configuration");

    let fault_store = store::mem_store::MemStore::new_db();

    let proxy = proxy::connection::Connection::new(
        config.redis_address.clone(),
        proxy::faulter::Faulter::new(fault_store.clone()),
        config.is_redis_tls_conn,
    )
    .expect("Error configuring proxy");

    let fault_config_server_future = tokio::spawn(async move {
        info!("Starting fault config server");
        fault_config_server::routes::run(fault_store)
            .await
            .expect("Failed to start the fault configuration server");
    });

    let proxy_listener_addr =
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), config.proxy_port);
    info!("Proxy listening on: {}", proxy_listener_addr);
    let listener = TcpListener::bind(&proxy_listener_addr)
        .await
        .expect("Error binding the proxy port");

    let proxy_future = tokio::spawn(async move {
        loop {
            let (socket, _addr) = match listener.accept().await {
                Ok((socket, addr)) => (socket, addr),
                Err(err) => {
                    error!("error accepting connection: {}", err);
                    continue;
                }
            };

            debug!("handling tcp connection");
            proxy
                .clone()
                .handle_connection(socket)
                .await
                .unwrap_or_else(|err| {
                    error!("error handling connection: {:?}", err);
                });
        }
    });

    let _ = join!(fault_config_server_future, proxy_future);
    Ok(())
}
