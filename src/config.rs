use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub redis: RedisConfig,
    pub server: ServerConfig,
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub logging: Logging,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Logging {
    #[serde(default)]
    pub level: String,
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            level: "debug".to_string(),
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

#[derive(Deserialize, Clone)]
pub struct ServerConfig {
    pub worker_count: u16,
    pub max_kline_count: u32,
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
