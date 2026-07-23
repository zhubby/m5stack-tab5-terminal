use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use longbridge::{
    Config, QuoteContext,
    quote::{PushEventDetail, SubFlags, TradeSessions},
};
use rust_decimal::prelude::ToPrimitive;

use crate::{
    config::WatchItem,
    error::AppError,
    models::{IntradayPoint, Quote, QuoteStatus},
    providers::QuoteProvider,
    state::AppState,
};

#[derive(Debug, Clone)]
pub struct LongbridgeQuoteProvider {
    watchlist: Vec<WatchItem>,
}

impl LongbridgeQuoteProvider {
    pub fn new(watchlist: Vec<WatchItem>) -> Self {
        Self { watchlist }
    }
}

#[async_trait]
impl QuoteProvider for LongbridgeQuoteProvider {
    async fn run(self: Box<Self>, state: AppState) -> Result<(), AppError> {
        if self.watchlist.is_empty() {
            state.set_provider_status("degraded: empty_watchlist").await;
            return Err(AppError::provider("Longbridge provider requires WATCHLIST"));
        }

        state.set_provider_status("connecting").await;
        let config =
            Arc::new(Config::from_apikey_env().map_err(|err| AppError::provider(err.to_string()))?);
        let (ctx, mut receiver) = QuoteContext::new(config);
        let symbols = self
            .watchlist
            .iter()
            .map(|item| item.provider_symbol.clone())
            .collect::<Vec<_>>();

        let mut previous_closes = HashMap::new();
        for snapshot in ctx
            .quote(symbols.clone())
            .await
            .map_err(|err| AppError::provider(err.to_string()))?
        {
            previous_closes.insert(
                snapshot.symbol.clone(),
                snapshot.prev_close.to_f64().unwrap_or_default(),
            );
            if let Some(item) = self
                .watchlist
                .iter()
                .find(|item| item.provider_symbol == snapshot.symbol)
            {
                let trade_status = format!("{:?}", snapshot.trade_status).to_ascii_lowercase();
                let status = quote_status_from_trade_status(&trade_status);
                state
                    .upsert_quote(Quote {
                        symbol: item.symbol.clone(),
                        name: item.name.clone(),
                        market: item.market,
                        last: snapshot.last_done.to_f64().unwrap_or_default(),
                        change: (snapshot.last_done - snapshot.prev_close)
                            .to_f64()
                            .unwrap_or_default(),
                        change_pct: pct_change(
                            snapshot.last_done.to_f64(),
                            snapshot.prev_close.to_f64(),
                        ),
                        open: snapshot.open.to_f64().unwrap_or_default(),
                        high: snapshot.high.to_f64().unwrap_or_default(),
                        low: snapshot.low.to_f64().unwrap_or_default(),
                        prev_close: snapshot.prev_close.to_f64().unwrap_or_default(),
                        volume: snapshot.volume.max(0) as u64,
                        turnover: snapshot.turnover.to_f64().unwrap_or_default(),
                        trade_status,
                        status,
                        quote_ts: chrono_from_time(snapshot.timestamp),
                        server_ts: Utc::now(),
                        stale: false,
                        stale_after_ms: state.config().stale_after.as_millis() as u64,
                    })
                    .await;
            }
        }

        ctx.subscribe(symbols, SubFlags::QUOTE)
            .await
            .map_err(|err| AppError::provider(err.to_string()))?;
        state.set_provider_status("running").await;

        let mut shutdown = state.shutdown_rx();
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        state.set_provider_status("stopped").await;
                        return Ok(());
                    }
                }
                event = receiver.recv() => {
                    match event {
                        Some(event) => {
                            if let PushEventDetail::Quote(push) = event.detail
                                && let Some(item) =
                                    self.watchlist
                                        .iter()
                                        .find(|item| item.provider_symbol == event.symbol)
                            {
                                    let prev_close = previous_closes.get(&event.symbol).copied().unwrap_or_default();
                                    let last = push.last_done.to_f64().unwrap_or_default();
                                    let timestamp = Utc
                                        .timestamp_opt(push.timestamp.unix_timestamp(), 0)
                                        .single()
                                        .unwrap_or_else(Utc::now);
                                    let trade_status =
                                        format!("{:?}", push.trade_status).to_ascii_lowercase();
                                    let status = quote_status_from_trade_status(&trade_status);
                                    state
                                        .upsert_quote(Quote {
                                            symbol: item.symbol.clone(),
                                            name: item.name.clone(),
                                            market: item.market,
                                            last,
                                            change: last - prev_close,
                                            change_pct: pct_change(Some(last), Some(prev_close)),
                                            open: push.open.to_f64().unwrap_or_default(),
                                            high: push.high.to_f64().unwrap_or_default(),
                                            low: push.low.to_f64().unwrap_or_default(),
                                            prev_close,
                                            volume: push.volume.max(0) as u64,
                                            turnover: push.turnover.to_f64().unwrap_or_default(),
                                            trade_status,
                                            status,
                                            quote_ts: timestamp,
                                            server_ts: Utc::now(),
                                            stale: false,
                                            stale_after_ms: state.config().stale_after.as_millis() as u64,
                                        })
                                        .await;
                            }
                        }
                        None => {
                            state.set_provider_status("degraded: stream_closed").await;
                            return Err(AppError::provider("Longbridge quote stream closed"));
                        }
                    }
                }
            }
        }
    }
}

pub async fn fetch_intraday(item: &WatchItem) -> Result<Vec<IntradayPoint>, AppError> {
    let config =
        Arc::new(Config::from_apikey_env().map_err(|err| AppError::provider(err.to_string()))?);
    let (ctx, _) = QuoteContext::new(config);
    let lines = ctx
        .intraday(item.provider_symbol.clone(), TradeSessions::Intraday)
        .await
        .map_err(|err| AppError::provider(err.to_string()))?;

    Ok(lines
        .into_iter()
        .map(|line| IntradayPoint {
            ts: chrono_from_time(line.timestamp),
            price: line.price.to_f64().unwrap_or_default(),
            avg_price: line.avg_price.to_f64().unwrap_or_default(),
            volume: line.volume.max(0) as u64,
            turnover: line.turnover.to_f64().unwrap_or_default(),
        })
        .collect())
}

fn quote_status_from_trade_status(status: &str) -> QuoteStatus {
    let normalized = status.to_ascii_lowercase();
    if normalized.contains("halt") || normalized.contains("suspend") {
        QuoteStatus::Suspended
    } else if normalized.contains("close") {
        QuoteStatus::MarketClosed
    } else {
        QuoteStatus::Normal
    }
}

fn pct_change(last: Option<f64>, prev_close: Option<f64>) -> f64 {
    let last = last.unwrap_or_default();
    let prev_close = prev_close.unwrap_or_default();

    if prev_close.abs() < f64::EPSILON {
        0.0
    } else {
        ((last - prev_close) / prev_close) * 100.0
    }
}

fn chrono_from_time(timestamp: time::OffsetDateTime) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(timestamp.unix_timestamp(), 0)
        .single()
        .unwrap_or_else(Utc::now)
}
