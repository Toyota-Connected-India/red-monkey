use env_logger::Env;
use log::debug;
use redis::{Client, Commands, Connection};
use std::time;

fn set_val(conn: &mut Connection, key: &str, val: i32) {
    let result = conn.set::<&str, i32, String>(key, val);
    match result {
        Ok(val) => {
            debug!("set key: {}; value: {}", key, val);
        }
        Err(e) => debug!("Error on setting key: {} value: {}: {}", key, val, e),
    }
}

fn get_val(conn: &mut Connection, key: &str) {
    debug!("About to send GET request");
    match conn.get::<&str, i32>(key) {
        Ok(val) => debug!("Task ID: {}", val),
        Err(e) => debug!("Error fetching key {:?}: {}", key, e),
    };
}

fn init_logger() {
    let env = Env::default()
        .filter_or("LOG_LEVEL", "debug")
        .write_style_or("LOG_STYLE", "always");
    env_logger::init_from_env(env);
}

fn main() {
    init_logger();

    let client = match Client::open("redis://127.0.0.1:6350") {
        Ok(c) => c,
        Err(e) => panic!("error creating redis client: {}", e),
    };

    let mut conn = match client.get_connection() {
        Ok(c) => c,
        Err(e) => panic!("error creating connection: {}", e),
    };

    let read_timeout = time::Duration::from_secs(6);
    let _ = conn.set_read_timeout(Some(read_timeout));

    let write_timeout = time::Duration::from_secs(2);
    let _ = conn.set_write_timeout(Some(write_timeout));

    set_val(&mut conn, "taskId", 7);
    get_val(&mut conn, "taskId");
}
