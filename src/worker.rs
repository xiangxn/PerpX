use crate::{
    handlers::trend_handler::process_consecutive_move,
    handlers::trend_handler::process_volatility_spike,
    helper::align_ts,
    types::{Interval, Kline, Message},
};
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::redis::RedisQueue;
use std::sync::Arc;

// ========== 核心逻辑 ==========
pub async fn worker(mut rx: mpsc::Receiver<Message>, max_kline_count: u32, queue: Arc<RedisQueue>) {
    let mut klines: HashMap<String, HashMap<Interval, Vec<Kline>>> = HashMap::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            Message::Ticker(t) => {
                let price: f64 = t.last_price.parse().unwrap_or(0.0);
                let volume: f64 = t.volume.parse().unwrap_or(0.0);
                let ts = t.event_time;

                let entry = klines.entry(t.symbol.clone()).or_insert_with(HashMap::new);

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
                                queue_clone
                            )
                            .await;
                        });

                        let queue_clone2 = queue.clone();
                        tokio::spawn(async move {
                            process_consecutive_move(symbol2, interval, closed_klines2,queue_clone2).await;
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
            Message::MarkPrice(m) => {
                // debug!(
                //     "MarkPrice: {} {} {}",
                //     m.symbol, m.funding_rate, m.next_funding_time
                // );
            }
        }
    }
}
