use redis::{Client, Commands, Connection};

fn set_val(conn: &mut Connection, key: &str, val: i32) {
    let _: () = conn.set(key, val).unwrap();
    println!("set key: {}; value: {}", key, val);
}

fn get_val(conn: &mut Connection, key: &str) {
    match conn.get::<&str, i32>(key) {
        Ok(val) => println!("Task ID: {}", val),
        Err(e) => println!("Error fetching key {:?}: {}", key, e),
    };
}

fn main() {
    let client = match Client::open("redis://127.0.0.1:6350") {
        Ok(c) => c,
        Err(e) => panic!("error creating redis client: {}", e),
    };

    let mut conn = match client.get_connection() {
        Ok(c) => c,
        Err(e) => panic!("error creating connection: {}", e),
    };

    set_val(&mut conn, "taskId", 7);
    get_val(&mut conn, "taskId");
}
