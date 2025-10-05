use crate::redis::RedisQueue;
use crate::types::{Event, EventType, Interval, Kline};
use serde_json::{json, to_string_pretty};
use std::sync::Arc;
use tracing::{debug, error, info};

// 异常波动
pub async fn process_volatility_spike(
    symbol: String,
    interval: Interval,
    klines: Vec<Kline>,
    turnover: String,
    queue: Arc<RedisQueue>,
) {
    debug!(
        "process_volatility_spike {:?} {:?} {}",
        symbol,
        interval,
        klines.len()
    );

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
        info!("New event: {}", to_string_pretty(&new_event).unwrap());
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
    turnover: String,
    queue: Arc<RedisQueue>,
) {
    debug!(
        "process_consecutive_move {:?} {:?} {}",
        symbol,
        interval,
        klines.len()
    );

    if klines.len() < 3 {
        // 最小3柱才计算逻辑
        return;
    }

    // 最大取10个周期
    let len = klines.len();
    // 取的长度：不超过 10，不少于 3
    let take_len = len.min(10).max(3);
    // 从后往前取
    let slice = &klines[len - take_len..];

    let mut count = 1;
    let mut iter = slice.iter().rev(); // 反向迭代
    let mut prev = iter.next().unwrap();

    // trend: +1 表示递增，-1 表示递减, 相等时方向不变
    let mut trend = 0;
    for curr in iter {
        if trend == 0 {
            if prev.close >= curr.close {
                trend = 1; // 正向递增趋势
            } else {
                trend = -1; // 正向递减趋势
            }
            count += 1;
        } else {
            match trend {
                1 if prev.close >= curr.close => count += 1,  // 一直递增
                -1 if prev.close <= curr.close => count += 1, // 一直递减
                _ => break,                                   // 趋势被破坏就结束
            }
        }
        prev = curr;
    }

    if count >= 3 {
        // 至少连续3个周期才发出事件
        let value = json!({
            "count": count.clone(),
            "turnover": turnover,
            "direction": trend
        })
        .as_object()
        .unwrap()
        .clone();
        let new_event = Event {
            event_type: EventType::ConsecutiveMove,
            symbol: symbol.to_string(),
            period: interval.to_string(),
            value,
            timestamp: klines.last().unwrap().start_ts,
        };
        info!("New event: {}", to_string_pretty(&new_event).unwrap());
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
            "{} 连续 {}个 {} 周期 {}",
            symbol,
            count,
            interval.to_string(),
            trend
        );
    }
}

pub async fn process_funding_rate(
    symbol: String,
    event_time: u64,
    funding_rate: String,
    next_funding_time: u64,
    queue: Arc<RedisQueue>,
) {
    debug!("process_funding_rate {:?} {}", symbol, funding_rate);
    let value = json!({
        "funding_rate": funding_rate.clone(),
        "next_funding_time": next_funding_time.clone()
    })
    .as_object()
    .unwrap()
    .clone();
    let new_event = Event {
        event_type: EventType::FundingRate,
        symbol: symbol.to_string(),
        period: "".to_string(),
        value,
        timestamp: event_time,
    };
    info!("New event: {}", to_string_pretty(&new_event).unwrap());
    // 写到redis
    if let Err(e) = queue
        .push("events", new_event.to_json().as_str(), None)
        .await
    {
        error!("failed to push event to redis: {:?}", e);
    }
}
