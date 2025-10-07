use crate::{
    config::FundingRateConfig, handlers::trend_handler::{
        process_consecutive_move, process_funding_rate, process_volatility_spike,
    }, helper::align_ts, types::{FundingRateLimit, Interval, Kline, Message}
};
use std::collections::{hash_map::Entry, HashMap};
use tokio::sync::mpsc;
use tracing::error;

use crate::redis::RedisQueue;
use std::sync::Arc;

// ========== 核心逻辑 ==========
pub async fn worker(
    mut rx: mpsc::Receiver<Message>,
    max_kline_count: u32,
    funding_rate_config: FundingRateConfig,
    queue: Arc<RedisQueue>,
) {
    let mut all_symbols: HashMap<String, HashMap<Interval, Vec<Kline>>> = HashMap::new();
    let mut send_rate: HashMap<String, FundingRateLimit> = HashMap::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            Message::Ticker(t) => {
                let price: f64 = t.last_price.parse().unwrap_or(0.0);
                let volume: f64 = t.volume.parse().unwrap_or(0.0);
                let ts = t.event_time;

                let entry = all_symbols
                    .entry(t.symbol.clone())
                    .or_insert_with(HashMap::new);

                for &interval in &[
                    Interval::Min5,
                    Interval::Min15,
                    Interval::Hour1,
                    Interval::Hour4,
                ] {
                    let aligned_ts = align_ts(ts, interval);
                    let klines = entry.entry(interval).or_insert(Vec::new());

                    if klines.is_empty() {
                        klines.push(Kline::new(aligned_ts, price, volume));
                    } else if klines.last().unwrap().start_ts != aligned_ts {
                        let symbol = t.symbol.clone();
                        let symbol2 = t.symbol.clone();
                        let closed_klines = klines.clone();
                        let closed_klines2 = klines.clone();
                        let closed_turnover = t.turnover.clone();
                        let queue_clone = queue.clone();

                        tokio::spawn(async move {
                            process_volatility_spike(
                                symbol,
                                interval,
                                closed_klines,
                                closed_turnover,
                                queue_clone,
                            )
                            .await;
                        });

                        let queue_clone2 = queue.clone();
                        let closed_turnover2 = t.turnover.clone();
                        tokio::spawn(async move {
                            process_consecutive_move(
                                symbol2,
                                interval,
                                closed_klines2,
                                closed_turnover2,
                                queue_clone2,
                            )
                            .await;
                        });
                        // 添加新kline
                        klines.push(Kline::new(aligned_ts, price, volume));
                        if klines.len() > max_kline_count as usize {
                            klines.drain(0..1);
                        }
                    } else {
                        klines.last_mut().unwrap().update(price, volume);
                    }
                }
            }
            Message::MarkPrice(m) => match m.funding_rate.parse::<f64>() {
                Ok(funding_rate) if funding_rate.abs() > funding_rate_config.min_funding_rate => {
                    let changed = match send_rate.entry(m.symbol.clone()) {
                        Entry::Vacant(e) => {
                            e.insert(FundingRateLimit {
                                time: m.event_time,
                                rate: funding_rate,
                            });
                            true
                        }
                        Entry::Occupied(mut e) => {
                            // 变化的值必须大于1e-5，并且时间间隔>=funding_rate_interval，否则不更新
                            if (e.get().rate - funding_rate).abs() > funding_rate_config.min_funding_rate_change
                                && m.event_time - e.get().time > funding_rate_config.funding_rate_interval * 1000
                            {
                                e.get_mut().rate = funding_rate;
                                e.get_mut().time = m.event_time;
                                true
                            } else {
                                false
                            }
                        }
                    };
                    if changed {
                        let symbol = m.symbol.clone();
                        let queue_clone = queue.clone();
                        let funding_rate = m.funding_rate.clone();
                        tokio::spawn(async move {
                            process_funding_rate(
                                symbol,
                                m.event_time,
                                funding_rate,
                                m.next_funding_time,
                                queue_clone,
                            )
                            .await;
                        });
                    }
                }
                Ok(_) => {
                    // debug!("{} Funding rate {}", m.symbol, m.funding_rate);
                }
                Err(e) => {
                    error!("funding_rate parse error: {}", e);
                }
            },
        }
    }
}
