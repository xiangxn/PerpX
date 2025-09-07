use anyhow::Result;
use fast_websocket_client::proxy::Proxy;
use fast_websocket_client::{ConnectionInitOptions, WebSocketBuilder};
use fxhash::hash64;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio;
use tokio::sync::mpsc;

mod config;
use crate::config::load_config;

// ========== 数据结构 ==========
#[derive(Debug, Deserialize, Clone)]
struct Ticker {
    e: String, // event type
    E: u64,    // eventTime
    s: String, // symbol
    c: String, // lastPrice
    v: String, // volume
}

#[derive(Debug, Deserialize, Clone)]
struct MarkPrice {
    e: String, // event type
    E: u64,    // eventTime
    s: String, // symbol
    r: String, // Funding rate
    T: u64,    // Next funding time
}

#[derive(Debug, Clone)]
enum Message {
    Ticker(Ticker),
    MarkPrice(MarkPrice),
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
enum Interval {
    Min5,
    Min15,
    Hour1,
    Hour4,
}

impl Interval {
    fn seconds(&self) -> u64 {
        match self {
            Interval::Min5 => 300,
            Interval::Min15 => 900,
            Interval::Hour1 => 3600,
            Interval::Hour4 => 14400,
        }
    }
}

#[derive(Debug, Clone)]
struct Kline {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    start_ts: u64,
}

impl Kline {
    fn new(ts: u64, price: f64, volume: f64) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume,
            start_ts: ts,
        }
    }

    fn update(&mut self, price: f64, volume: f64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += volume;
    }
}

#[derive(Debug, Clone)]
struct Strategy {
    symbol: String,
    interval: Interval,
    pct_change_gt: f64, // e.g. 5.0 means > 5%
}

// ========== 核心逻辑 ==========

fn align_ts(ts: u64, interval: Interval) -> u64 {
    ts - (ts % (interval.seconds() * 1000)) // Binance E 是毫秒
}

fn process_closed_kline(symbol: &str, interval: Interval, kline: &Kline, strategies: &[Strategy]) {
    let pct = (kline.close - kline.open) / kline.open * 100.0;

    for s in strategies {
        if s.symbol == symbol && s.interval == interval && pct.abs() > s.pct_change_gt {
            println!(
                "⚡ ALERT: {} {} 涨幅 {:.2}% (O:{:.2} C:{:.2})",
                symbol,
                format!("{:?}", interval),
                pct,
                kline.open,
                kline.close
            );
            // TODO: 发消息到 TG / Webhook
        }
    }
}

async fn worker(mut rx: mpsc::Receiver<Message>, strategies: Vec<Strategy>) {
    let mut klines: HashMap<String, HashMap<Interval, Kline>> = HashMap::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            Message::Ticker(t) => {
                println!("Ticker: {} {} {} {}", t.s, t.c, t.v, t.E);

                let price: f64 = t.c.parse().unwrap_or(0.0);
                let volume: f64 = t.v.parse().unwrap_or(0.0);
                let ts = t.E;

                let entry = klines.entry(t.s.clone()).or_insert_with(HashMap::new);

                for &interval in &[
                    Interval::Min5,
                    Interval::Min15,
                    Interval::Hour1,
                    Interval::Hour4,
                ] {
                    let aligned_ts = align_ts(ts, interval);

                    let kline = entry
                        .entry(interval)
                        .or_insert_with(|| Kline::new(aligned_ts, price, volume));

                    // 如果时间不对齐，说明进入新周期 -> 收盘旧的
                    if kline.start_ts != aligned_ts {
                        let closed = kline.clone();
                        process_closed_kline(&t.s, interval, &closed, &strategies);

                        // 替换为新k线
                        *kline = Kline::new(aligned_ts, price, volume);
                    } else {
                        kline.update(price, volume);
                    }
                    // println!(
                    //     "{} {} {} {} {}",
                    //     kline.start_ts, kline.open, kline.high, kline.low, kline.close
                    // )
                }
            }
            Message::MarkPrice(m) => {
                println!("MarkPrice: {} {} {}", m.s, m.r, m.T);
            }
        }
    }
}

// Dispatcher：把 symbol hash 到固定 worker
fn assign_worker(symbol: &str, worker_count: usize) -> usize {
    (hash64(symbol.as_bytes()) % worker_count as u64) as usize
}

/// Called when the WebSocket opens
async fn handle_open() {
    println!("[OPEN] WebSocket connection established.");
}

/// Called when the WebSocket closes
async fn handle_close() {
    println!("[CLOSE] WebSocket connection closed.");
}

async fn handle_error(e: String) {
    eprintln!("[ERROR] {}", e);
}

async fn handle_message(
    worker_txs: Arc<Vec<tokio::sync::mpsc::Sender<Message>>>,
    worker_count: usize,
    msg: String,
) {
    if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(&msg) {
        match json_value["stream"].as_str() {
            Some("!miniTicker@arr") => {
                let data = json_value["data"].take();
                if let Ok(tickers) = serde_json::from_value::<Vec<Ticker>>(data) {
                    for ticker in tickers {
                        let idx = assign_worker(&ticker.s, worker_count);
                        let _ = worker_txs[idx].try_send(Message::Ticker(ticker));
                    }
                }
            }
            Some("!markPrice@arr") => {
                let data = json_value["data"].take();
                if let Ok(mark_prices) = serde_json::from_value::<Vec<MarkPrice>>(data) {
                    for mark_price in mark_prices {
                        let idx = assign_worker(&mark_price.s, worker_count);
                        let _ = worker_txs[idx].try_send(Message::MarkPrice(mark_price));
                    }
                }
            }
            _ => eprintln!("未知事件类型: {}", msg),
        }
    }
}

// ========== 主入口 ==========
#[tokio::main]
async fn main() -> Result<()> {
    let cfg =
        load_config("config.toml").map_err(|e| anyhow::anyhow!("config load error: {}", e))?;
    let worker_count = cfg.server.worker_count as usize; // 2核CPU可以设为2~4
    let mut worker_txs = Vec::new();

    // 策略配置
    let strategies = vec![
        Strategy {
            symbol: "BTCUSDT".into(),
            interval: Interval::Min15,
            pct_change_gt: 1.0,
        },
        Strategy {
            symbol: "ETHUSDT".into(),
            interval: Interval::Min5,
            pct_change_gt: 5.0,
        },
    ];

    // 创建 worker pool
    for i in 0..worker_count {
        let (tx, rx) = mpsc::channel::<Message>(10000);
        worker_txs.push(tx);
        let s_clone = strategies.clone();
        tokio::spawn(async move {
            println!("🚀 Worker {} started", i);
            worker(rx, s_clone).await;
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
        .connect("wss://fstream.binance.com/stream?streams=!miniTicker@arr/!markPrice@arr")
        .await?;

    client.await_shutdown().await;
    Ok(())
}
