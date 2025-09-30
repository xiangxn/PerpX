use crate::types::{Interval};
use fxhash::hash64;

pub fn align_ts(ts: u64, interval: Interval) -> u64 {
    ts - (ts % (interval.seconds() * 1000)) // Binance E 是毫秒
}

// 计算hash,把 symbol hash 到固定 worker
pub fn assign_worker(symbol: &str, worker_count: usize) -> usize {
    (hash64(symbol.as_bytes()) % worker_count as u64) as usize
}