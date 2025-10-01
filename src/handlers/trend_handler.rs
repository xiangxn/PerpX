use crate::redis::RedisQueue;
use crate::types::{Event, EventType, Interval, Kline};
use serde_json::{json, to_string_pretty};
use std::sync::Arc;
use tracing::{debug, error};

// 异常波动
pub async fn process_volatility_spike(
    symbol: String,
    interval: Interval,
    klines: Vec<Kline>,
    turnover: String,
    queue: Arc<RedisQueue>,
) {
    debug!("{:?} {:?} {}", symbol, interval, klines.len());

    if klines.len() < 4 {
        // 最小4柱才计算逻辑
        return;
    }
    let history = &klines[klines.len() - 4..klines.len() - 1];
    let current = klines.last().unwrap();

    let current_amp = (current.high - current.low) / current.open;
    let prev_amps = history
        .iter()
        .map(|k| (k.high - k.low) / k.open)
        .collect::<Vec<f64>>();
    let avg_prev_amp = prev_amps.iter().sum::<f64>() / prev_amps.len() as f64;

    let direction = if current.close > history.last().unwrap().close {
        1
    } else {
        -1
    };

    if current_amp > 0.0001 && current_amp > avg_prev_amp * 2.0 {
        // 发出事件
        let value = json!({
            "amplitude": current_amp,
            "avg_amplitude": avg_prev_amp,
            "volume": current.volume,
            "turnover": turnover,
            "direction": direction
        })
        .as_object()
        .unwrap()
        .clone();
        let new_event = Event {
            symbol: symbol.to_string(),
            event_type: EventType::VolatilitySpike,
            period: interval.to_string(),
            value,
            timestamp: current.start_ts,
        };
        debug!("New event: {}", to_string_pretty(&new_event).unwrap());
        // 写到redis
        if let Err(e) = queue
            .push("events", new_event.to_json().as_str(), None)
            .await
        {
            error!("failed to push event to redis: {:?}", e);
        }
    } else {
        // 只输出日志
        debug!(
            "{:?} 振幅: {:.2} 过去平均: {:.2}",
            symbol,
            current_amp * 100.0,
            avg_prev_amp * 100.0
        )
    }
}

// 连续 N 个周期涨/跌
pub async fn process_consecutive_move(
    symbol: String,
    interval: Interval,
    klines: Vec<Kline>,
    redis: Arc<RedisQueue>,
) {
    // TODO:
}
