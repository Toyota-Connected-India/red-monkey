use ::redis::ConnectionLike;
use log::{debug, error};
use r2d2_redis::{r2d2, RedisConnectionManager};
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::ops::DerefMut;
use std::str;

pub struct ConnectionError;

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Connection Error:")
    }
}

pub struct Connection {
    pub redis_server_addr: String,
    pool: r2d2::Pool<r2d2_redis::RedisConnectionManager>,
}

impl Connection {
    pub fn new(redis_server_addr: &str) -> Result<Connection, ConnectionError> {
        let manager = RedisConnectionManager::new(redis_server_addr.to_string()).unwrap();
        let pool = r2d2::Pool::builder().max_size(30).build(manager).unwrap();

        Ok(Connection {
            redis_server_addr: redis_server_addr.to_string(),
            pool,
        })
    }

    pub fn handle_connection(&self, mut stream: TcpStream) {
        let mut redis_command = [0; 1024];

        let n = stream.read(&mut redis_command);
        match n {
            Ok(n) => debug!("read {:?} bytes from the request", n),
            Err(e) => error!("error reading request: {:?}", e),
        }

        //let pool = self.pool.clone();
        let mut conn = self.pool.get().unwrap();
        let redis_conn = conn.deref_mut();

        let mut redis_value = redis_conn
            .req_packed_command_raw_resp(&redis_command)
            .unwrap();

        let mut server_resp_buff = [0; 1024];

        let n = redis_value.read(&mut server_resp_buff);
        if let Ok(n) = n {
            debug!("read {:?} bytes from the server response", n);
        }

        stream.write_all(&server_resp_buff).unwrap();
        stream.flush().unwrap();

        debug!("wrote server response in the client stream");
    }
}
