#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    pub redis_address: String,
    pub is_redis_tls_conn: bool,
}

pub fn get_config() -> Result<Config, envy::Error> {
    match envy::from_env::<Config>() {
        Ok(config) => Ok(config),
        Err(e) => Err(e),
    }
}

fn default_proxy_port() -> u16 {
    6350
}
