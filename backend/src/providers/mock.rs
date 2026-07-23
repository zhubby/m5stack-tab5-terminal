use async_trait::async_trait;
use chrono::{Duration, Utc};
use tokio::time::{MissedTickBehavior, interval};

use crate::{
    config::WatchItem,
    error::AppError,
    models::{IntradayPoint, Quote, QuoteStatus},
    providers::QuoteProvider,
    state::AppState,
};

#[derive(Debug, Clone)]
pub struct MockQuoteProvider {
    watchlist: Vec<WatchItem>,
}

impl MockQuoteProvider {
    pub fn new(watchlist: Vec<WatchItem>) -> Self {
        Self { watchlist }
    }
}

#[async_trait]
impl QuoteProvider for MockQuoteProvider {
    async fn run(self: Box<Self>, state: AppState) -> Result<(), AppError> {
        state.set_provider_status("running").await;
        let mut shutdown = state.shutdown_rx();
        let mut tick = interval(state.config().mock_interval);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut sequence = 0_u64;

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        state.set_provider_status("stopped").await;
                        return Ok(());
                    }
                }
                _ = tick.tick() => {
                    sequence += 1;
                    for (index, item) in self.watchlist.iter().enumerate() {
                        let quote = mock_quote(item, index, sequence, state.config().stale_after.as_millis() as u64);
                        state.upsert_quote(quote).await;
                    }
                }
            }
        }
    }
}

pub fn mock_quote(item: &WatchItem, index: usize, sequence: u64, stale_after_ms: u64) -> Quote {
    let base = match item.symbol.as_str() {
        "000001.SH" => 3100.0,
        "399001.SZ" => 9800.0,
        "600519.SH" => 1680.0,
        "00700.HK" => 380.0,
        "09988.HK" => 78.0,
        _ => 100.0 + (index as f64 * 7.5),
    };
    let drift = (((sequence + index as u64) % 17) as f64 - 8.0) * 0.03;
    let last = round2(base * (1.0 + drift / 100.0));
    let prev_close = base;
    let change = round2(last - prev_close);
    let change_pct = round2((change / prev_close) * 100.0);
    let open = round2(base * (1.0 + (((index as f64 % 5.0) - 2.0) * 0.08) / 100.0));
    let high = round2(open.max(last) * 1.003);
    let low = round2(open.min(last) * 0.997);
    let now = Utc::now();

    Quote {
        symbol: item.symbol.clone(),
        name: item.name.clone(),
        market: item.market,
        last,
        change,
        change_pct,
        open,
        high,
        low,
        prev_close,
        volume: 1_000_000 + sequence * 1_000 + index as u64 * 10_000,
        turnover: round2(last * 1_000_000.0),
        trade_status: "normal".to_string(),
        status: QuoteStatus::Normal,
        quote_ts: now,
        server_ts: now,
        stale: false,
        stale_after_ms,
    }
}

pub fn mock_intraday(item: &WatchItem, quote: &Quote) -> Vec<IntradayPoint> {
    let base = if quote.prev_close.abs() > f64::EPSILON {
        quote.prev_close
    } else if quote.last.abs() > f64::EPSILON {
        quote.last
    } else {
        100.0
    };
    let end = Utc::now();
    let start = end - Duration::minutes(119);
    let mut cumulative_price = 0.0;

    (0..120)
        .map(|index| {
            let progress = index as f64 / 119.0;
            let target_move = quote.last - base;
            let wave = ((index as f64 + item.symbol.len() as f64) / 8.0).sin() * base * 0.0018;
            let price = round2(base + target_move * progress + wave);
            cumulative_price += price;
            let avg_price = round2(cumulative_price / (index as f64 + 1.0));
            let volume = 2_000 + ((index * 137 + item.symbol.len() * 211) % 14_000) as u64;
            IntradayPoint {
                ts: start + Duration::minutes(index as i64),
                price,
                avg_price,
                volume,
                turnover: round2(price * volume as f64),
            }
        })
        .collect()
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
