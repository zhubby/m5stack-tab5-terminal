use async_trait::async_trait;
use chrono::Utc;
use tokio::time::{MissedTickBehavior, interval};

use crate::{
    config::WatchItem,
    error::AppError,
    models::{Quote, QuoteStatus},
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
    let now = Utc::now();

    Quote {
        symbol: item.symbol.clone(),
        name: item.name.clone(),
        market: item.market,
        last,
        change,
        change_pct,
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

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
