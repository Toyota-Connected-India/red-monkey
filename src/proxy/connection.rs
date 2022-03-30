use crate::proxy::faulter::{Context, Faulter, RequestAction};
use crate::proxy::resp_util::get_host_name;
use anyhow::anyhow;
use bytes::Bytes;
use futures::stream::{Stream, StreamExt, TryStreamExt};
use std::borrow::Borrow;
use std::net::ToSocketAddrs;
use tokio::{
    io,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, Result as TokioResult},
    net::TcpStream,
};
use tokio_native_tls::{native_tls::TlsConnector, TlsStream};
use tokio_util::codec;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Connection is the core of the proxy.
///
/// Handles client's connection as follows.
///
/// - Checks if it has to apply any fault by checking the request payload against the
/// configured faults.  
/// - If the request matches with any fault, it executes it.
/// - If no fault matches with the request payload, the request will be proxied to the origin
/// server without any changes to the request payload.
#[derive(Clone)]
pub struct Connection {
    server_addr: String,
    faulter: Faulter,
    is_tls_conn: bool,
}

trait AsyncReadWrite: AsyncRead + AsyncWrite + Unpin + Send {}

/// The TcpStream and TlsStream implements the trait AsyncReadWrite which is a super trait of
/// tokio::io::AsyncRead and tokio::io::AsyncWrite.
impl AsyncReadWrite for TcpStream {}
impl AsyncReadWrite for TlsStream<TcpStream> {}

/// into_bytes_stream converts the given object (implements AsyncRead) into Stream of Bytes.
///
/// # Arguments
/// - R implements tokio::io::AsyncRead
fn into_bytes_stream<R>(r: R) -> impl Stream<Item = TokioResult<Bytes>>
where
    R: AsyncRead,
{
    codec::FramedRead::new(r, codec::BytesCodec::new()).map_ok(|bytes| bytes.freeze())
}

impl Connection {
    /// Creates a new Connection object
    ///
    /// # To be improved
    ///
    /// - Currently, we create a new TCP connection to the origin server for each non-fault request.
    /// But, this could turn out to be a costly op on a large number of requests. This can be improved
    /// by TCP connection pooling.
    pub fn new(
        server_addr: String,
        faulter: Faulter,
        is_tls_conn: bool,
    ) -> Result<Self, anyhow::Error> {
        Ok(Connection {
            server_addr,
            faulter,
            is_tls_conn,
        })
    }

    /// Creates a new TCP server stream object.
    ///
    /// # Errors
    ///
    /// - When the server of server_addr is not reachable, this method will return error like
    /// `ConnectionRefused`.
    async fn new_tcp_stream(&self) -> Result<Box<dyn AsyncReadWrite>, anyhow::Error> {
        let tcp_stream = TcpStream::connect(&self.server_addr).await?;
        Ok(Box::new(tcp_stream))
    }

    /// Creates a new TLS over TCP server stream object.
    ///
    /// # Errors
    /// - When the server of server_addr is not reachable, this method will return error like
    /// `ConnectionRefused`.
    async fn new_tls_stream(&self) -> Result<Box<dyn AsyncReadWrite>, anyhow::Error> {
        let server_addr = self
            .server_addr
            .as_str()
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                anyhow!(
                    "error failed to resolve server address: {}",
                    self.server_addr,
                )
            })?;

        let tcp_stream = TcpStream::connect(&server_addr).await?;
        let tls_connector = TlsConnector::builder().build()?;
        let tls_connector = tokio_native_tls::TlsConnector::from(tls_connector);

        let host_name = get_host_name(&self.server_addr.clone())?;
        let tls_stream = tls_connector.connect(&host_name, tcp_stream).await?;

        Ok(Box::new(tls_stream))
    }

    /// Based on the Connection configuration, returns a server connection object.
    ///
    /// If TLS connection is enabled, this method returns a TLS connection over TCP stream to the
    /// server.
    /// Else, it returns a raw TCP connection stream to the server.
    async fn create_server_stream(&self) -> Result<Box<dyn AsyncReadWrite>, anyhow::Error> {
        let stream = if self.is_tls_conn {
            info!("establishing tls connection to {}", self.server_addr);
            self.new_tls_stream().await?
        } else {
            info!("establishing tcp connection to {}", self.server_addr);
            self.new_tcp_stream().await?
        };

        Ok(stream)
    }

    /// handle is the core of the proxy connection handling. It handles the connection between
    /// the client and the origin server. When no faults are configured, handle will act as a typical
    /// proxy; forwards all the requests to the server.
    ///
    /// Based on the endpoint, handle will decided to establish a tcp connection or a tls
    /// connection over the tcp stream. In the connection pipeline, it checks if the request
    /// matches with any fault plan. If so, appropriate fault (delay / custom error) will be applied.
    ///
    #[tracing::instrument(
    name = "Handling connection",
        skip(self),
        fields(
            request_id = %Uuid::new_v4(),
        )
    )]
    pub async fn handle(self, mut inbound_stream: TcpStream) -> Result<(), anyhow::Error> {
        let (client_read_stream, mut client_write_stream) = inbound_stream.split();

        // convert the AsyncRead into a stream of byte buffers
        let mut client_stream = into_bytes_stream(client_read_stream).map(|buf| buf);

        let mut req_bytes: Bytes = Bytes::new();
        if let Some(data) = client_stream.next().await {
            match data {
                Ok(data) => req_bytes = data,
                Err(err) => {
                    error!("error converting request bytes into streams: {}", err);
                    return Err(err.into());
                }
            }
        };

        let req_payload_str = std::str::from_utf8(&req_bytes)?;
        debug!("request payload bytes: {:?}", req_payload_str);

        let fault = self
            .faulter
            .check_fault(req_payload_str)
            .await
            .map_err(|err| {
                error!("error checking fault for a given request: {}", err);
                err
            })?;

        let mut ctx = Context {
            client_tcp_write_stream: &mut client_write_stream,
        };

        match self.faulter.execute_fault(&mut ctx, fault).await? {
            RequestAction::Exit => {
                info!("exiting  request processing");
                return Ok(());
            }
            RequestAction::Fallthrough => {
                info!("continuing request processing");
            }
        }

        let server_stream = self.create_server_stream().await.map_err(|err| {
            error!("error creating server stream: {:?}", err);
            err
        })?;

        let (mut server_read_stream, mut server_write_stream) = tokio::io::split(server_stream);

        let client_to_server = async {
            io::copy(&mut req_bytes.borrow(), &mut server_write_stream).await?;
            info!("request proxied to the server");
            server_write_stream.shutdown().await
        };

        let server_to_client = async {
            io::copy(&mut server_read_stream, &mut client_write_stream).await?;
            info!("response proxied to the client");
            client_write_stream.shutdown().await
        };

        let _ = tokio::try_join!(client_to_server, server_to_client)?;

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        proxy,
        store::{
            self,
            fault_store::{Fault, FaultVariants, DB},
        },
    };
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Instant;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpStream;

    /// An individual test in the connection module can run upto 2 TCP servers - mock origin server
    /// and red-monkey server. By default, cargo runs the tests in parallel, hence we are using an
    /// Atomic usize integer that can be safely shared across threads to give an exclusive port number
    /// to each TCP server with the use of `fetch_add` method.  
    static PORT: AtomicUsize = AtomicUsize::new(0);

    /// next_test_ip4 returns a socket address with an increasing port number that internally
    /// uses an `AtomicUsize` for the port number. Hence, calling this function from different
    /// threads is guarenteed to return a mutually exclusive increasing port number.
    pub fn next_test_ip4() -> SocketAddr {
        let port = PORT.fetch_add(1, Ordering::SeqCst) as u16 + 10000;
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port))
    }

    /// The mock origin server is a simple echo server. The `handle` method of
    /// `Connection` is agnostic of the application layer protocol of the origin server. Hence to
    /// test the proxy module, the echo origin server should suffice.
    pub fn run_mock_origin_server(origin_server_addr: SocketAddr) {
        debug!("binding origin server to {} address", origin_server_addr);
        let listener = TcpListener::bind(&origin_server_addr).unwrap();

        thread::spawn(move || {
            for (mut socket, _addr) in listener.accept() {
                let mut buf = [0; 1028];
                thread::spawn(move || {
                    match socket.read(&mut buf) {
                        Ok(n) => {
                            socket.write(&buf[0..n]).unwrap();
                        }
                        Err(err) => {
                            panic!("error reading data from tcp socket: {}", err);
                        }
                    };
                });
            }
        });
    }

    /// Runs a mock proxy server asynchronously that calls the `handle` method of `Connection`, which
    /// is the core handler of the proxy.  
    async fn run_red_monkey_server(red_monkey_server_addr: SocketAddr, fault_store: DB) {
        let origin_server_addr = next_test_ip4();
        run_mock_origin_server(origin_server_addr);

        debug!(
            "binding red-monkey server to {} address",
            red_monkey_server_addr
        );
        let listener = tokio::net::TcpListener::bind(&red_monkey_server_addr)
            .await
            .unwrap();

        let connection = Connection::new(
            origin_server_addr.to_string(),
            proxy::faulter::Faulter::new(fault_store),
            false,
        )
        .unwrap();

        debug!("listening for client connections");
        tokio::spawn(async move {
            loop {
                let (socket, _addr) = listener.accept().await.unwrap();
                debug!("accepted a connection");
                let connection = connection.clone();

                tokio::spawn(async move {
                    connection.handle(socket).await.unwrap();
                    debug!("handled the connection");
                });
            }
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_proxy_without_fault() {
        env_logger::init();

        let red_monkey_server_addr = next_test_ip4();
        let fault_store = store::mem_store::MemStore::new_db();

        run_red_monkey_server(red_monkey_server_addr, fault_store).await;

        let mut stream = TcpStream::connect(red_monkey_server_addr).await.unwrap();

        let write_buffer = b"*3\r\n$3\r\nset\r\n$5\r\nmykey\r\n$1\r\n1\r\n";
        stream.write_all(write_buffer).await.unwrap();

        let mut read_buffer = [0; 32];
        match stream.read(&mut read_buffer).await {
            Ok(n) => {
                assert_eq!(n, write_buffer.len());
                assert_eq!(
                    read_buffer[0..n],
                    *b"*3\r\n$3\r\nset\r\n$5\r\nmykey\r\n$1\r\n1\r\n",
                );
            }
            Err(err) => {
                panic!("error reading data from tcp socket: {}", err);
            }
        };
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_proxy_drop_fault() {
        let red_monkey_server_addr = next_test_ip4();
        let fault_store = store::mem_store::MemStore::new_db();

        let fault = Fault {
            name: "drop_conn_for_set_cmd".to_string(),
            description: Some("SET drop connection error".to_string()),
            fault_type: FaultVariants::DropConn,
            error_msg: None,
            duration: None,
            command: "SET".to_string(),
            last_modified: None,
        };

        fault_store
            .write()
            .await
            .store(&fault.name, &fault)
            .unwrap();

        run_red_monkey_server(red_monkey_server_addr, fault_store).await;

        let mut stream = TcpStream::connect(red_monkey_server_addr).await.unwrap();
        let write_buffer = b"*3\r\n$3\r\nset\r\n$5\r\nmykey\r\n$1\r\n1\r\n";
        stream.write_all(write_buffer).await.unwrap();

        let mut read_buffer = [0; 32];
        match stream.read(&mut read_buffer).await {
            Ok(n) => {
                debug!("read {} bytes", n);
                assert_eq!(n, 0);
            }
            Err(err) => {
                panic!("error reading data from tcp socket: {}", err);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_proxy_custom_error_fault() {
        let red_monkey_server_addr = next_test_ip4();
        let fault_store = store::mem_store::MemStore::new_db();

        let fault = Fault {
            name: "set_custom_err".to_string(),
            description: Some("SET custom error".to_string()),
            fault_type: FaultVariants::Error,
            error_msg: Some("SET FAILED".to_string()),
            duration: None,
            command: "SET".to_string(),
            last_modified: None,
        };

        fault_store
            .write()
            .await
            .store(&fault.name, &fault)
            .unwrap();

        run_red_monkey_server(red_monkey_server_addr, fault_store).await;

        let mut stream = TcpStream::connect(red_monkey_server_addr).await.unwrap();
        let write_buffer = b"*3\r\n$3\r\nset\r\n$5\r\nmykey\r\n$1\r\n1\r\n";
        stream.write_all(write_buffer).await.unwrap();

        let mut read_buffer = [0; 32];
        match stream.read(&mut read_buffer).await {
            Ok(n) => {
                assert_eq!(read_buffer[0..n], *b"-SET FAILED\r\n");
            }
            Err(err) => {
                panic!("error reading data from tcp socket: {}", err);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_proxy_delay_fault() {
        let red_monkey_server_addr = next_test_ip4();
        let fault_store = store::mem_store::MemStore::new_db();

        let fault = Fault {
            name: "delay_fault".to_string(),
            description: Some("SET delay fault".to_string()),
            fault_type: FaultVariants::Delay,
            error_msg: None,
            duration: Some(20),
            command: "SET".to_string(),
            last_modified: None,
        };

        fault_store
            .write()
            .await
            .store(&fault.name, &fault)
            .unwrap();

        run_red_monkey_server(red_monkey_server_addr, fault_store).await;

        let mut stream = TcpStream::connect(red_monkey_server_addr).await.unwrap();
        let write_buffer = b"*3\r\n$3\r\nset\r\n$5\r\nmykey\r\n$1\r\n1\r\n";
        stream.write_all(write_buffer).await.unwrap();

        let start = Instant::now();

        let mut read_buffer = [0; 32];
        match stream.read(&mut read_buffer).await {
            Ok(n) => {
                let duration = start.elapsed();
                assert_eq!(
                    read_buffer[0..n],
                    *b"*3\r\n$3\r\nset\r\n$5\r\nmykey\r\n$1\r\n1\r\n"
                );
                assert_eq!(duration.as_millis() < 20, false);
            }
            Err(err) => {
                panic!("error reading data from tcp socket: {}", err);
            }
        }
    }
}
