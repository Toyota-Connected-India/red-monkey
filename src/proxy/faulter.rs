use crate::proxy::resp_util;
use crate::store::fault_store::{Fault, FaultVariants, DB};
use std::{str, time};
use tokio::time::sleep;
use tokio::{io, io::AsyncWriteExt};
use tracing::{debug, error, info};

/// Faulter implements the logic that determines whether any of the configured fault is to be
/// executed for a request. Also, it takes care of executing the matched or chosen fault.
#[derive(Clone)]
pub struct Faulter {
    fault_store: DB,
}

/// Context holds the relevant object that is required to execute fault of certain type.
///
/// Note: As we add more fault configuration like injecting faults based on client IP address,
/// those data can be held in the Context struct.
pub struct Context<'a, 'b> {
    pub client_tcp_write_stream: &'a mut tokio::net::tcp::WriteHalf<'b>,
}

/// RequestAction tells what the request processor (proxy handler) should do after a fault is
/// executed and the action differs based on the fault variant.
#[derive(Debug, PartialEq)]
pub enum RequestAction {
    Exit,
    Fallthrough,
}

impl Faulter {
    pub fn new(fault_store: DB) -> Self {
        Faulter { fault_store }
    }

    /// check_fault checks if the request matches with any fault configuration.
    ///
    /// # Arguments
    /// req_body - request body
    #[tracing::instrument(name = "Check fault", skip(self, req_body))]
    pub async fn check_fault(&self, req_body: &str) -> Result<Option<Fault>, anyhow::Error> {
        let redis_command: String;
        let result = resp_util::decode(req_body);

        match result {
            Ok(val) => match resp_util::fetch_redis_command(val) {
                Ok(command) => {
                    debug!("redis command: {}", command);
                    redis_command = command;
                }
                Err(err) => {
                    error!("error fetching redis command from req: {:?}", err);
                    return Err(err);
                }
            },

            Err(err) => {
                error!("error decoding request body: {:?}", err);
                return Err(err);
            }
        };

        let fault_store = self.fault_store.read().await;

        let fault_config = fault_store.get_by_redis_cmd(redis_command.as_str());
        match fault_config {
            Some(fault) => Ok(Some(fault)),
            None => Ok(None),
        }
    }

    /// Executes the fault that is passed as an argument.
    ///
    /// # Arguments
    /// - ctx - Context holds the write half of the client TCP stream
    /// - fault - Optional `Fault`. If the fault is optional it means no fault matched to be executed
    #[tracing::instrument(name = "Executing fault", skip(self, ctx))]
    pub async fn execute_fault<'a, 'b, 'c>(
        &self,
        ctx: &'a mut Context<'b, 'c>,
        fault: Option<Fault>,
    ) -> Result<RequestAction, anyhow::Error> {
        let fault = match fault {
            Some(f) => f,
            None => {
                return Ok(RequestAction::Fallthrough);
            }
        };

        match fault.fault_type {
            FaultVariants::DropConn => {
                info!("executing drop fault: dropping the client connection");
                ctx.client_tcp_write_stream.shutdown().await?;
                Ok(RequestAction::Exit)
            }
            FaultVariants::Delay => {
                info!("executing delay fault");
                execute_delay_fault(fault.duration).await;
                Ok(RequestAction::Fallthrough)
            }
            FaultVariants::Error => {
                info!("executing error fault");
                execute_error_fault(ctx, fault).await?;
                Ok(RequestAction::Exit)
            }
        }
    }
}

/// Injecting delay fault is Sleeping for a given duration in an asynchronous tokio way.
///
/// Sleeping for x duration shouldn't block the thread which could turn out to be fatal during
/// large number of sleep fault execution. `tokio::time::sleep` helps us to sleep asynchronously,
/// such that the current thread of fault execution won't be blocked. More about tokio sleep can be
/// found here - <https://docs.rs/tokio/0.3.1/tokio/time/fn.sleep.html>.
#[tracing::instrument(name = "Injecting delay fault")]
pub async fn execute_delay_fault(sleep_duration: Option<u64>) {
    if let Some(sleep_duration) = sleep_duration {
        let sleep_duration = time::Duration::from_millis(sleep_duration);

        info!("Sleeping for {:?}", sleep_duration);
        sleep(sleep_duration).await;
    };
}

/// Executes the given custom error fault.
///
/// - The error message will be RESP encoded.
/// - The encoded error message is then written in the client TCP write direction.
/// - The client TCP  write half is closed.
#[tracing::instrument(name = "Applying error fault", skip(ctx))]
pub async fn execute_error_fault<'a, 'b, 'c>(
    ctx: &'a mut Context<'b, 'c>,
    fault: Fault,
) -> Result<(), anyhow::Error> {
    let encoded_err_msg = resp_util::encode_error_message(
        fault
            .error_msg
            .ok_or_else(|| Box::new(FaulterErrors::EncodeErrMsgError))?,
    )?;

    let server_to_client = async {
        io::copy(
            &mut String::from_utf8_lossy(&encoded_err_msg)
                .to_string()
                .as_str()
                .as_bytes(),
            &mut ctx.client_tcp_write_stream,
        )
        .await?;

        debug!("error value wrote to the client");
        ctx.client_tcp_write_stream.shutdown().await
    };

    server_to_client.await?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum FaulterErrors {
    #[error("Error decoding request body to RESP values")]
    EncodeErrMsgError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::connection::tests::{next_test_ip4, run_mock_origin_server};
    use crate::store;
    use crate::store::fault_store::DB;
    use chrono::{Duration, Utc};
    use std::time::Instant;
    use tokio::io::{AsyncReadExt, ErrorKind};
    use tokio::net::TcpStream;

    async fn get_mock_fault_store() -> DB {
        let mock_faults = vec![
            Fault {
                name: "delay 1 second".to_string(),
                description: Some("inject a delay of 1 second".to_string()),
                fault_type: FaultVariants::Delay,
                duration: Some(1000),
                error_msg: None,
                command: "GET".to_string(),
                last_modified: Some(Utc::now()),
            },
            Fault {
                name: "SET Error".to_string(),
                description: Some("inject set error".to_string()),
                fault_type: FaultVariants::Error,
                duration: None,
                error_msg: Some("SET ERROR".to_string()),
                command: "SET".to_string(),
                last_modified: Some(Utc::now() + Duration::minutes(1)),
            },
            Fault {
                name: "drop_conn_for_ping_cmd".to_string(),
                description: Some("PING error".to_string()),
                fault_type: FaultVariants::DropConn,
                error_msg: None,
                duration: None,
                command: "PING".to_string(),
                last_modified: None,
            },
        ];

        let fault_store = store::mem_store::MemStore::new_db();

        for fault in mock_faults {
            fault_store
                .write()
                .await
                .store(fault.name.as_str(), &fault)
                .unwrap();
        }

        fault_store
    }

    #[tokio::test]
    async fn test_check_fault() {
        let fault_store = get_mock_fault_store().await;
        let faulter = Faulter::new(fault_store);

        let res = faulter
            .check_fault("*3\r\n$3\r\nset\r\n$4\r\nkey1\r\n$8\r\nvalue100\r\n")
            .await;

        assert_eq!(res.is_ok(), true);
        let fault = res.unwrap().unwrap();

        assert_eq!(fault.name, "SET Error".to_string());
        assert_eq!(fault.fault_type, FaultVariants::Error);
        assert_eq!(fault.command, "SET".to_string());
    }

    #[tokio::test]
    async fn test_check_fault_no_match() {
        let fault_store = store::mem_store::MemStore::new_db();
        let faulter = Faulter::new(fault_store);

        let res = faulter
            .check_fault("*3\r\n$3\r\nset\r\n$4\r\nkey1\r\n$8\r\nvalue100\r\n")
            .await;

        assert_eq!(res.is_ok(), true);
        assert_eq!(res.unwrap(), None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_delay_fault() {
        let fault_store = get_mock_fault_store().await;
        let faulter = Faulter::new(fault_store);

        let mock_server_addr = next_test_ip4();
        debug!("mock server address: {}", mock_server_addr);

        run_mock_origin_server(mock_server_addr);

        let mut stream = TcpStream::connect(mock_server_addr.to_string())
            .await
            .unwrap();
        let (_, mut write_stream) = stream.split();

        let mut ctx = Context {
            client_tcp_write_stream: &mut write_stream,
        };

        let fault = faulter
            .check_fault("*2\r\n$3\r\nget\r\n$4\r\nkey1\r\n")
            .await
            .unwrap();

        let start = Instant::now();
        let action = faulter.execute_fault(&mut ctx, fault).await;
        let duration = start.elapsed();

        assert_eq!(action.is_ok(), true);
        assert_eq!(action.unwrap(), RequestAction::Fallthrough);

        debug!("elapsed duration is: {:?}", duration.as_secs());

        assert_eq!(duration.as_millis() < 1000, false);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_error_fault() {
        let fault_store = get_mock_fault_store().await;
        let faulter = Faulter::new(fault_store);

        let mock_server_addr = next_test_ip4();
        debug!("mock server address: {}", mock_server_addr);

        run_mock_origin_server(mock_server_addr);

        let mut stream = TcpStream::connect(mock_server_addr.to_string())
            .await
            .unwrap();
        let (mut read_stream, mut write_stream) = stream.split();

        let mut ctx = Context {
            client_tcp_write_stream: &mut write_stream,
        };

        let fault = faulter
            .check_fault("*3\r\n$3\r\nset\r\n$4\r\nkey1\r\n$8\r\nvalue100\r\n")
            .await
            .unwrap();

        let action = faulter.execute_fault(&mut ctx, fault).await;
        assert_eq!(action.is_ok(), true);
        assert_eq!(action.unwrap(), RequestAction::Exit);

        let mut read_buffer = [0; 32];
        match read_stream.read(&mut read_buffer).await {
            Ok(n) => {
                assert_eq!(read_buffer[0..n], *b"-SET ERROR\r\n");

                // The write() method should fail by BrokenPipe error because the client write
                // stream is expected to be closed on the execution of the error fault.
                if let Err(e) = write_stream.write(b"").await {
                    assert_eq!(e.kind(), ErrorKind::BrokenPipe)
                }
            }
            Err(err) => {
                panic!("error reading data from tcp socket: {}", err);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_execute_drop_fault() {
        let fault_store = get_mock_fault_store().await;
        let faulter = Faulter::new(fault_store);

        let mock_server_addr = next_test_ip4();
        debug!("mock server address: {}", mock_server_addr);

        run_mock_origin_server(mock_server_addr);

        let mut stream = TcpStream::connect(mock_server_addr.to_string())
            .await
            .unwrap();
        let (mut read_stream, mut write_stream) = stream.split();

        let mut ctx = Context {
            client_tcp_write_stream: &mut write_stream,
        };

        let fault = faulter.check_fault("*1\r\n$4\r\nping\r\n").await.unwrap();
        let action = faulter.execute_fault(&mut ctx, fault).await;
        assert_eq!(action.is_ok(), true);
        assert_eq!(action.unwrap(), RequestAction::Exit);

        let mut read_buffer = [0; 32];
        match read_stream.read(&mut read_buffer).await {
            Ok(n) => {
                assert_eq!(read_buffer[0..n], *b"");

                // The write() method should fail by BrokenPipe error because the client write
                // stream is expected to be closed on the execution of the error fault.
                if let Err(e) = write_stream.write(b"").await {
                    assert_eq!(e.kind(), ErrorKind::BrokenPipe)
                }
            }
            Err(err) => {
                panic!("error reading data from tcp socket: {}", err);
            }
        }
    }
}
