use crate::proxy::faulter::Faulter;

use bytes::Bytes;
use futures::{
    self,
    stream::{Stream, StreamExt, TryStreamExt},
};
use log::debug;

use tokio::io;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncRead, Result};
use tokio::net::TcpStream;
use tokio_util::codec;
use tokio_util::io::StreamReader;

pub type Error = Box<dyn ::std::error::Error>;

#[derive(Clone)]
pub struct Connection {
    redis_server_addr: String,
    faulter: Faulter,
}

unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

fn into_bytes_stream<R>(r: R) -> impl Stream<Item = Result<Bytes>>
where
    R: AsyncRead,
{
    codec::FramedRead::new(r, codec::BytesCodec::new()).map_ok(|bytes| bytes.freeze())
}

impl Connection {
    pub fn new(redis_server_addr: String, faulter: Faulter) -> Self {
        Connection {
            redis_server_addr,
            faulter,
        }
    }

    pub async fn handle_connection(&self, mut inbound_stream: TcpStream) {
        let mut outbound_stream = TcpStream::connect(self.redis_server_addr.as_str())
            .await
            .unwrap();

        let (client_read_inbound, mut client_write_inbound) = inbound_stream.split();
        let (mut server_read_outbound, mut server_write_outbound) = outbound_stream.split();

        // convert the AsyncRead into a stream of byte buffers
        let client_read_stream = into_bytes_stream(client_read_inbound).map(|buf| {
            debug!("request payload {:?}", buf);

            match &buf {
                Ok(request_payload) => {
                    let request_payload = std::str::from_utf8(request_payload).unwrap();
                    self.faulter.apply_fault(request_payload).unwrap();
                }

                Err(_) => {}
            }

            buf
        });

        // convert it back to AsyncRead so we can pass it to io::copy
        let mut new_client_read_inbound = StreamReader::new(client_read_stream);

        let client_to_server = async {
            io::copy(&mut new_client_read_inbound, &mut server_write_outbound).await?;
            debug!("request proxied to redis server");
            server_write_outbound.shutdown().await
        };

        let server_to_client = async {
            io::copy(&mut server_read_outbound, &mut client_write_inbound).await?;
            debug!("response sent back to the client");
            client_write_inbound.shutdown().await
        };

        tokio::try_join!(client_to_server, server_to_client).unwrap();
    }
}
