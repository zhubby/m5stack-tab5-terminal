use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Market {
    Cn,
    Hk,
}

impl Market {
    pub fn infer(symbol: &str) -> Self {
        if symbol.to_ascii_uppercase().ends_with(".HK") {
            Self::Hk
        } else {
            Self::Cn
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStatus {
    Normal,
    Stale,
    Offline,
    MarketClosed,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub name: String,
    pub market: Market,
    pub last: f64,
    pub change: f64,
    pub change_pct: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub prev_close: f64,
    pub volume: u64,
    pub turnover: f64,
    pub trade_status: String,
    pub status: QuoteStatus,
    pub quote_ts: DateTime<Utc>,
    pub server_ts: DateTime<Utc>,
    pub stale: bool,
    pub stale_after_ms: u64,
}

impl Quote {
    pub fn status(&self) -> QuoteStatus {
        if self.stale {
            return QuoteStatus::Stale;
        }

        match self.status {
            QuoteStatus::Normal => {
                if self.trade_status.eq_ignore_ascii_case("suspended") {
                    QuoteStatus::Suspended
                } else if self.trade_status.eq_ignore_ascii_case("market_closed") {
                    QuoteStatus::MarketClosed
                } else {
                    QuoteStatus::Normal
                }
            }
            other => other,
        }
    }

    pub fn with_freshness(mut self, now: DateTime<Utc>, stale_after: std::time::Duration) -> Self {
        let age_is_stale = now
            .signed_duration_since(self.server_ts)
            .to_std()
            .is_ok_and(|age| age > stale_after);
        self.stale = self.stale || age_is_stale;
        self.status = self.status();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntradayPoint {
    pub ts: DateTime<Utc>,
    pub price: f64,
    pub avg_price: f64,
    pub volume: u64,
    pub turnover: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteDetailResponse {
    pub symbol: String,
    pub quote: Quote,
    pub intraday: Vec<IntradayPoint>,
    pub server_ts: DateTime<Utc>,
    pub cached: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamRequest {
    DetailRequest { request_id: u64, symbol: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistResponse {
    pub items: Vec<WatchlistItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub symbol: String,
    pub name: String,
    pub market: Market,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpsertWatchItemRequest {
    pub symbol: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteWatchItemResponse {
    pub deleted: bool,
    pub items: Vec<WatchlistItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub provider: String,
    pub provider_status: String,
    pub quote_count: usize,
    pub last_quote_ts: Option<DateTime<Utc>>,
    pub server_ts: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamMessage {
    Snapshot {
        quotes: Vec<Quote>,
    },
    Quote {
        quote: Quote,
    },
    Status {
        status: String,
        server_ts: DateTime<Utc>,
    },
    Error {
        message: String,
        server_ts: DateTime<Utc>,
    },
    Detail {
        request_id: u64,
        symbol: String,
        quote: Quote,
        intraday: Vec<IntradayPoint>,
        server_ts: DateTime<Utc>,
        cached: bool,
    },
    DetailError {
        request_id: u64,
        symbol: String,
        message: String,
        server_ts: DateTime<Utc>,
    },
}
