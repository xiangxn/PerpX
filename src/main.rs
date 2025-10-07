use anyhow::Result;
use fast_websocket_client::proxy::Proxy;
use fast_websocket_client::{ConnectionInitOptions, WebSocketBuilder};
use rustis::client::Client;
use std::sync::Arc;
use tokio;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::EnvFilter;

mod config;
mod handlers;
mod helper;
mod redis;
mod types;
mod worker;
use crate::config::load_config;
use crate::helper::assign_worker;
use crate::redis::RedisQueue;
use crate::types::{MarkPrice, Message, Ticker};
use crate::worker::worker;

/// Called when the WebSocket opens
async fn handle_open() {
    info!("[OPEN] WebSocket connection established.");
}

/// Called when the WebSocket closes
async fn handle_close() {
    info!("[CLOSE] WebSocket connection closed.");
}

async fn handle_error(e: String) {
    error!("{}", e);
}

async fn handle_message(
    worker_txs: Arc<Vec<tokio::sync::mpsc::Sender<Message>>>,
    worker_count: usize,
    msg: String,
) {
    if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(&msg) {
        match json_value["stream"].as_str() {
            Some("!ticker@arr") => {
                let data = json_value["data"].take();
                if let Ok(tickers) = serde_json::from_value::<Vec<Ticker>>(data) {
                    for ticker in tickers {
                        let idx = assign_worker(&ticker.symbol, worker_count);
                        let _ = worker_txs[idx].try_send(Message::Ticker(ticker));
                    }
                }
            }
            Some("!markPrice@arr") => {
                let data = json_value["data"].take();
                if let Ok(mark_prices) = serde_json::from_value::<Vec<MarkPrice>>(data) {
                    for mark_price in mark_prices {
                        let idx = assign_worker(&mark_price.symbol, worker_count);
                        let _ = worker_txs[idx].try_send(Message::MarkPrice(mark_price));
                    }
                }
            }
            _ => warn!("未知事件类型: {}", msg),
        }
    }
}

// ========== 主入口 ==========
#[tokio::main]
async fn main() -> Result<()> {
    let cfg =
        load_config("config.toml").map_err(|e| anyhow::anyhow!("config load error: {}", e))?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(cfg.logging.level))
        .with_timer(ChronoLocal::rfc_3339()) // ISO 8601 格式
        .init();

    let worker_count = cfg.server.worker_count as usize; // 2核CPU可以设为2~4
    let mut worker_txs = Vec::new();

    // 创建 redis 客户端
    let url = format!(
        "redis://{}:{}@{}:{}",
        cfg.redis.user, cfg.redis.password, cfg.redis.host, cfg.redis.port
    );
    let redis_client = Client::connect(url).await?;
    let redis_queue = Arc::new(RedisQueue::new(redis_client, cfg.server.redis_data_expire));

    // 创建 worker pool
    for i in 0..worker_count {
        let (tx, rx) = mpsc::channel::<Message>(10000);
        worker_txs.push(tx);
        let redis = redis_queue.clone();
        let funding_rate_config = cfg.funding_rate.clone();
        tokio::spawn(async move {
            info!("🚀 Worker {} started", i);
            worker(rx, cfg.server.max_kline_count, funding_rate_config, redis).await;
        });
    }

    let ws = if let Some(proxy_cfg) = &cfg.proxy {
        let options = ConnectionInitOptions::default().proxy(Some(Proxy::Socks5 {
            addr: proxy_cfg.addr.clone(),
            auth: None,
        }));
        WebSocketBuilder::new().with_options(options)
    } else {
        WebSocketBuilder::new()
    };

    let worker_txs = Arc::new(worker_txs);
    let worker_txs_clone = worker_txs.clone();
    let client = ws
        .on_open(move |_| handle_open())
        .on_close(|_| handle_close())
        .on_error(move |e| handle_error(e))
        .on_message(move |message| handle_message(worker_txs_clone.clone(), worker_count, message))
        .connect("wss://fstream.binance.com/stream?streams=!ticker@arr/!markPrice@arr")
        .await?;

    client.await_shutdown().await;
    Ok(())
}
