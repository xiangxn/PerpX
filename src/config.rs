use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub redis: RedisConfig,
    pub server: ServerConfig,
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub logging: Logging,
    pub funding_rate: FundingRateConfig,
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
    pub redis_data_expire: usize,
}

#[derive(Deserialize, Clone)]
pub struct FundingRateConfig {
    pub min_funding_rate: f64,        // 最小资金费率 0.0001(0.01%)
    pub min_funding_rate_change: f64, // 资金费率变化的最小幅度 0.0001(0.01%)
    pub funding_rate_interval: u64,   // 资金费率事件的最小间隔，单位秒
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
