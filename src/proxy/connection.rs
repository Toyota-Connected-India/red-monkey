use r2d2_redis;
use r2d2_redis::{r2d2, redis, RedisConnectionManager};
use std::fmt;
use std::io::prelude::*;
use std::net::TcpStream;
use std::ops::DerefMut;
use std::thread;

pub struct Connection {
    pub redis_server_addr: String,
    pool: r2d2::Pool,
}

pub struct ConnectionError;

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Connection Error:")
    }
}

pub fn new(redis_server_addr: &String) -> Result<Connection, ConnectionError> {
    let manager = RedisConnectionManager::new(redis_server_addr.to_string()).unwrap();
    let _pool = r2d2::Pool::builder().build(manager).unwrap();

    Ok(Connection {
        redis_server_addr: redis_server_addr.to_string(),
    })
}

impl Connection {
    pub fn handle_connection(&self, mut stream: TcpStream) {
        let mut buffer = [0; 1024];
        stream.read(&mut buffer).unwrap();
        let redis_command = String::from_utf8_lossy(&buffer[..]).to_string();

        let manager = RedisConnectionManager::new(self.redis_server_addr.to_string()).unwrap();
        let pool = r2d2::Pool::builder().build(manager).unwrap();

        let mut handles = vec![];
        let max: i32 = 10;

        for _i in 0..max {
            let pool = pool.clone();
            let cmd = redis_command.to_string();

            handles.push(thread::spawn(move || {
                let mut conn = pool.get().unwrap();

                // TODO: Fix the RESP error
                let _reply = redis::cmd(&cmd).query::<String>(conn.deref_mut()).unwrap();

                // Alternatively, without deref():
                //let reply = redis::cmd(&cmd).query::<String>(&mut *conn).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
