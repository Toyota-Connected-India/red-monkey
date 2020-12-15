#[derive(Deserialize, Debug)]
pub struct Config {
    pub proxy_listen_port: String,
    pub redis_address: String,
    pub is_tls_on: bool,
    pub fault_api_listen_port: String,
}

pub fn get_config() -> Result<Config, envy::Error> {
    match envy::from_env::<Config>() {
        Ok(config) => return Ok(config),
        Err(e) => return Err(e),
    }
}
