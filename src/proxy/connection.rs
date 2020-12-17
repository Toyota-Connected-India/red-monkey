use r2d2_redis::{r2d2, redis, RedisConnectionManager};
use redis::ConnectionLike;
use std::fmt;
use std::io::prelude::*;
use std::io::Write;
use std::net::TcpStream;
use std::ops::DerefMut;

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
    pub fn new(redis_server_addr: &String) -> Result<Conn, ConnectionError> {
        let manager = RedisConnectionManager::new(redis_server_addr.to_string()).unwrap();
        let pool = r2d2::Pool::builder().max_size(10).build(manager).unwrap();

        Ok(Conn {
            redis_server_addr: redis_server_addr.to_string(),
            pool,
        })
    }

    pub fn handle_connection(&self, mut stream: TcpStream) {
        let mut redis_command = [0; 1024];
        stream.read(&mut redis_command).unwrap();

        let pool = self.pool.clone();
        let mut conn = pool.clone().get().unwrap();
        let redis_conn = conn.deref_mut();

        let redis_value = redis_conn.req_packed_command(&redis_command).unwrap();

        let result: String = redis::from_redis_value(&redis_value).unwrap();
        println!("result {:?}, {:?}", result, result.chars().nth(0).unwrap());

        stream.write(result.as_str().as_bytes()).unwrap();
    }
}
