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

pub struct Conn {
    pub redis_server_addr: String,
    pool: r2d2::Pool<r2d2_redis::RedisConnectionManager>,
}

impl Conn {
    pub fn new(redis_server_addr: &str) -> Result<Conn, ConnectionError> {
        let manager = RedisConnectionManager::new(redis_server_addr.to_string()).unwrap();
        let pool = r2d2::Pool::builder().max_size(10).build(manager).unwrap();

        Ok(Conn {
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

        if let Ok(redis_command) = str::from_utf8(&mut redis_command) {
            debug!("redis request: {:?}", redis_command);
        }

        let pool = self.pool.clone();

        let mut conn = pool.get().unwrap();
        let redis_conn = conn.deref_mut();

        let mut redis_value = redis_conn
            .req_packed_command_raw_resp(&redis_command)
            .unwrap();

        // TODO: Buffer size 1024 is not safe
        // Should read the response from server until EOF
        let mut server_resp_buff = [0; 1024];

        let n = redis_value.read(&mut server_resp_buff);
        match n {
            Ok(n) => debug!("read {:?} bytes", n),
            Err(_) => {}
        };

        if let Ok(s) = str::from_utf8(&mut server_resp_buff) {
            debug!("server response: {:?}", s);
        }

        stream.write(&mut server_resp_buff).unwrap();
        stream.flush().unwrap();

        debug!("connection closed");
    }
}
