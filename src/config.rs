#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_proxy_port")]
    pub proxy_port: String,
    #[serde(default = "default_redis_address")]
    pub redis_address: String,
    #[serde(default = "default_tls_value")]
    pub is_tls_on: bool,
}

pub fn get_config() -> Result<Config, envy::Error> {
    match envy::from_env::<Config>() {
        Ok(config) => Ok(config),
        Err(e) => Err(e),
    }
}

fn default_proxy_port() -> String {
    String::from("127.0.0.1:6350")
}

fn default_tls_value() -> bool {
    false
}

fn default_redis_address() -> String {
    String::from("127.0.0.1:6379")
}
