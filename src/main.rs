use env_logger::Env;
use log::{debug, error, info};
use std::net::TcpListener;
use std::process;
use std::sync::Arc;
use std::thread;

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

fn main() {
    init_logger();

    let config = config::get_config().unwrap_or_else(|e| {
        error!("Failed to parse environment variables : {}", e);
        process::exit(0);
    });
    debug!("env configs: {:?}", config);

    let listener = TcpListener::bind(&config.proxy_listen_port).unwrap();
    info!("Listening on port: {}", config.proxy_listen_port);

    let proxy = match proxy::connection::Connection::new(&config.redis_address) {
        Ok(proxy) => proxy,
        Err(e) => panic!("error creating new proxy: {}", e),
    };

    let proxy = Arc::new(proxy);

    for stream in listener.incoming() {
        debug!("connection established!");
        let proxy = proxy.clone();
        let stream = stream.unwrap();

        thread::spawn(move || {
            proxy.handle_connection(stream);
            debug!("connection closed");
        });
    }

    let _fault_store = crate::store::mem_store::new();
}
