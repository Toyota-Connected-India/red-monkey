#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    pub redis_address: String,
    pub is_redis_tls_conn: bool,
    #[serde(default = "default_fault_config_server_port")]
    pub fault_config_server_port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_proxy_port() -> u16 {
    6350
}

fn default_fault_config_server_port() -> u16 {
    8000
}

fn default_log_level() -> String {
    "INFO".to_string()
}

pub fn get_config() -> Result<Config, envy::Error> {
    match envy::from_env::<Config>() {
        Ok(config) => Ok(config),
        Err(e) => Err(e),
    }
}
