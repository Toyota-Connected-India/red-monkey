use anyhow::anyhow;
use std::borrow::Borrow;
use tokio_native_tls::{native_tls::TlsConnector, TlsStream};
use url::Url;

use crate::proxy::faulter::{Faulter, FaulterValue};

use bytes::Bytes;
use futures::stream::{Stream, StreamExt, TryStreamExt};
use log::{debug, info};

use std::net::ToSocketAddrs;
use tokio::{
    io,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, Result as TResult},
    net::TcpStream,
};
use tokio_util::codec;

#[derive(Clone)]
pub struct Connection {
    redis_addr: String,
    faulter: Faulter,
    is_tls_conn: bool,
}

trait AsyncReadWrite: AsyncRead + AsyncWrite + Unpin + Send {}

impl AsyncReadWrite for TcpStream {}
impl AsyncReadWrite for TlsStream<TcpStream> {}

fn into_bytes_stream<R>(r: R) -> impl Stream<Item = TResult<Bytes>>
where
    R: AsyncRead,
{
    codec::FramedRead::new(r, codec::BytesCodec::new()).map_ok(|bytes| bytes.freeze())
}

impl Connection {
    pub fn new(
        redis_addr: String,
        faulter: Faulter,
        is_tls_conn: bool,
    ) -> Result<Self, anyhow::Error> {
        Ok(Connection {
            redis_addr,
            faulter,
            is_tls_conn,
        })
    }

    #[allow(dead_code)]
    async fn new_tcp_stream(&self) -> Result<Box<dyn AsyncReadWrite>, anyhow::Error> {
        let tcp_stream = TcpStream::connect(&self.redis_addr).await?;
        Ok(Box::new(tcp_stream))
    }

    async fn new_tls_stream(&self) -> Result<Box<dyn AsyncReadWrite>, anyhow::Error> {
        let redis_addr = self
            .redis_addr
            .as_str()
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                anyhow!(
                    "error failed to resolve redis_server_addr: {}",
                    self.redis_addr,
                )
            })?;

        let tcp_stream = TcpStream::connect(&redis_addr).await?;
        let tls_connector = TlsConnector::builder().build()?;
        let tls_connector = tokio_native_tls::TlsConnector::from(tls_connector);

        let redis_host_name = get_host_name(&self.redis_addr.clone())?;
        info!("redis host name: {}", redis_host_name);

        let tls_stream = tls_connector.connect(&redis_host_name, tcp_stream).await?;

        Ok(Box::new(tls_stream))
    }

    pub async fn handle(self, mut inbound_stream: TcpStream) -> Result<(), anyhow::Error> {
        let (client_read_stream, mut client_write_stream) = inbound_stream.split();

        let stream = if self.is_tls_conn {
            info!("establishing tls connection");
            self.new_tls_stream().await?
        } else {
            info!("establishing tcp connection");
            self.new_tcp_stream().await?
        };

        let (mut server_read_stream, mut server_write_stream) = tokio::io::split(stream);

        // convert the AsyncRead into a stream of byte buffers
        let mut client_stream =
            into_bytes_stream(client_read_stream).map(|buf| async { self.check_fault(buf?).await });

        let mut fault_err_msg: Option<anyhow::Error> = None;
        let mut req_bytes: Bytes = Bytes::from("");

        if let Some(data) = client_stream.next().await {
            match data.await {
                Ok(data) => req_bytes = data,
                Err(err) => fault_err_msg = Some(err),
            }
        };

        debug!("request bytes: {:?}; {:?}", req_bytes, fault_err_msg);

        if let Some(err_msg) = fault_err_msg {
            let server_to_client = async {
                io::copy(
                    &mut err_msg.to_string().as_str().as_bytes(),
                    &mut client_write_stream,
                )
                .await?;

                debug!("error value wrote to the client");
                client_write_stream.shutdown().await?;

                Ok::<(), anyhow::Error>(())
            };

            server_to_client.await?;
            return Ok(());
        };

        let client_to_server = async {
            io::copy(&mut req_bytes.borrow(), &mut server_write_stream).await?;
            debug!("request proxied to redis server");
            server_write_stream.shutdown().await
        };

        let server_to_client = async {
            io::copy(&mut server_read_stream, &mut client_write_stream).await?;
            debug!("response sent back to the client");
            client_write_stream.shutdown().await
        };

        let _ = tokio::try_join!(client_to_server, server_to_client)?;

        Ok(())
    }

    async fn check_fault(&self, request_payload: Bytes) -> Result<Bytes, anyhow::Error> {
        let mut fault_err_msg = String::new();

        let request_payload_str = std::str::from_utf8(&request_payload)?;
        let response = self.faulter.check_for_fault(request_payload_str).await?;

        if let FaulterValue::Value(v) = response {
            fault_err_msg = String::from_utf8_lossy(&v).to_string();
        }

        if !fault_err_msg.is_empty() {
            return Err(anyhow!(fault_err_msg));
        }

        Ok(request_payload)
    }
}

fn get_host_name(redis_server_addr: &str) -> Result<String, anyhow::Error> {
    let mut parsed_redis_url = Url::parse(redis_server_addr)?;

    if parsed_redis_url.host_str() == None {
        parsed_redis_url = Url::parse(&format!("redis://{}", redis_server_addr))?;
    }

    let host_name = parsed_redis_url
        .host_str()
        .ok_or_else(|| anyhow!("Error fetching the hostname from redis address"))?;

    Ok(host_name.to_string())
}
