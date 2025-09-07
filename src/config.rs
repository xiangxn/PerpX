use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Deserialize, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Deserialize, Clone)]
pub struct ServerConfig {
    pub worker_count: u16,
}

#[derive(Deserialize, Clone)]
pub struct ProxyConfig {
    pub addr: String,
}

// 方便主程序直接调用一个函数加载配置
use std::fs;

pub fn load_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
