use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

// ========== 数据结构 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct Ticker {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "c")]
    pub last_price: String, // 最新成交价格
    #[serde(rename = "Q")]
    pub volume: String, // 最新成交价格上的成交量
    #[serde(rename = "q")]
    pub turnover: String, // 24小时成交额
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct MarkPrice {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "r")]
    pub funding_rate: String,
    #[serde(rename = "T")]
    pub next_funding_time: u64,
}

#[derive(Debug, Clone)]
pub enum Message {
    Ticker(Ticker),
    MarkPrice(MarkPrice),
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum Interval {
    Min5,
    Min15,
    Hour1,
    Hour4,
}

impl Interval {
    pub fn seconds(&self) -> u64 {
        match self {
            Interval::Min5 => 300,
            Interval::Min15 => 900,
            Interval::Hour1 => 3600,
            Interval::Hour4 => 14400,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Interval::Min5 => "5m".to_string(),
            Interval::Min15 => "15m".to_string(),
            Interval::Hour1 => "1h".to_string(),
            Interval::Hour4 => "4h".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Kline {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub start_ts: u64,
}

impl Kline {
    pub fn new(ts: u64, price: f64, volume: f64) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume,
            start_ts: ts,
        }
    }

    pub fn update(&mut self, price: f64, volume: f64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += volume;
    }
}

#[derive(Debug, Clone)]
pub struct Strategy {
    pub symbol: String,
    pub interval: Interval,
    pub pct_change_gt: f64, // e.g. 5.0 means > 5%
}

// 事件枚举
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Serialize)]
pub enum EventType {
    ConsecutiveMove, // 连续 N 个周期涨/跌
    VolatilitySpike, // 异常波动
}

// 事件数据结构
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub symbol: String,
    pub event_type: EventType,
    pub period: String,            // "1h" / "5m"
    pub value: Map<String, Value>, // 实际计算出的指标结果
    pub timestamp: u64,
}

impl Event {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize Event to JSON")
    }
}
