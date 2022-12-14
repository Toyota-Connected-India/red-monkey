use crate::fault_config_server::handler::*;
use crate::store::fault_store::DB;
use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use std::net::TcpListener;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing::info;
use tracing_actix_web::TracingLogger;

pub async fn run(port: u16, fault_store: DB) -> Result<(), anyhow::Error> {
    let server_listener_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
    let listener = TcpListener::bind(server_listener_addr)?;

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/fault", web::post().to(store_fault))
            .route("/fault/{fault_name}", web::get().to(get_fault))
            .route("/faults", web::get().to(get_all_faults))
            .route("/fault/{fault_name}", web::delete().to(delete_fault))
            .route("/faults", web::delete().to(delete_all_faults))
            .app_data(Data::new(fault_store.clone()))
    })
    .shutdown_timeout(2)
    .listen(listener)?
    .run();

    info!("Fault config server listening on: {}", server_listener_addr);

    server.await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_initialization() {
        let fault_store = crate::store::mem_store::MemStore::new_db();

        tokio::spawn(async move {
            run(9999, fault_store).await.unwrap();
        });
    }
}
