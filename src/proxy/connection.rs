use std::error::Error;
use std::fmt;

use log::debug;
use tokio::io;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub struct ConnectionError;

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[Connection Error]")
    }
}

pub async fn handle_connection(
    mut inbound_stream: TcpStream,
    redis_server_addr: String,
) -> Result<(), Box<dyn Error>> {
    let mut outbound_stream = TcpStream::connect(redis_server_addr.clone()).await?;

    let (mut read_inbound, mut write_inbound) = inbound_stream.split();
    let (mut read_outbound, mut write_outbound) = outbound_stream.split();

    let client_to_server = async {
        io::copy(&mut read_inbound, &mut write_outbound).await?;
        debug!("request proxied to redis server");
        write_outbound.shutdown().await
    };

    let server_to_client = async {
        io::copy(&mut read_outbound, &mut write_inbound).await?;
        debug!("response sent back to client");
        write_inbound.shutdown().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}
