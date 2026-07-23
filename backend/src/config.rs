use std::{env, net::SocketAddr, time::Duration};

use crate::models::{Market, WatchlistItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteProviderKind {
    Mock,
    Longbridge,
}

impl QuoteProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Longbridge => "longbridge",
        }
    }
}

impl TryFrom<&str> for QuoteProviderKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_str() {
            "mock" => Ok(Self::Mock),
            "longbridge" => Ok(Self::Longbridge),
            other => Err(format!("unsupported QUOTE_PROVIDER `{other}`")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub provider: QuoteProviderKind,
    pub watchlist: Vec<WatchItem>,
    pub stale_after: Duration,
    pub mock_interval: Duration,
    pub device_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchItem {
    pub symbol: String,
    pub provider_symbol: String,
    pub name: String,
    pub market: Market,
}

impl WatchItem {
    pub fn new(symbol: impl Into<String>, name: impl Into<String>, market: Market) -> Self {
        let symbol = symbol.into();
        Self {
            provider_symbol: normalize_provider_symbol(&symbol),
            symbol,
            name: name.into(),
            market,
        }
    }
}

impl From<&WatchItem> for WatchlistItem {
    fn from(item: &WatchItem) -> Self {
        Self {
            symbol: item.symbol.clone(),
            name: item.name.clone(),
            market: item.market,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .map_err(|err| format!("invalid BIND_ADDR: {err}"))?;
        let provider = env::var("QUOTE_PROVIDER")
            .unwrap_or_else(|_| "mock".to_string())
            .as_str()
            .try_into()?;
        let stale_after = Duration::from_secs(read_u64_env("STALE_AFTER_SECS", 20)?);
        let mock_interval = Duration::from_millis(read_u64_env("MOCK_INTERVAL_MS", 3000)?);
        let watchlist = parse_watchlist(&env::var("WATCHLIST").unwrap_or_default())?;
        let device_token = env::var("DEVICE_TOKEN")
            .ok()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());

        Ok(Self {
            bind_addr,
            provider,
            watchlist: if watchlist.is_empty() {
                default_watchlist()
            } else {
                watchlist
            },
            stale_after,
            mock_interval,
            device_token,
        })
    }
}

fn read_u64_env(name: &str, default: u64) -> Result<u64, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|err| format!("invalid {name}: {err}"))
        })
        .unwrap_or(Ok(default))
}

pub fn parse_watchlist(value: &str) -> Result<Vec<WatchItem>, String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let parts = entry.split(':').map(str::trim).collect::<Vec<_>>();
            let symbol = parts
                .first()
                .copied()
                .filter(|part| !part.is_empty())
                .ok_or_else(|| format!("invalid WATCHLIST entry `{entry}`"))?;
            let name = parts
                .get(1)
                .copied()
                .filter(|part| !part.is_empty())
                .unwrap_or(symbol);

            Ok(WatchItem::new(symbol, name, Market::infer(symbol)))
        })
        .collect()
}

fn normalize_provider_symbol(symbol: &str) -> String {
    match symbol.split_once('.') {
        Some((code, market)) if market.eq_ignore_ascii_case("HK") => {
            let normalized = code.trim_start_matches('0');
            let normalized = if normalized.is_empty() {
                "0"
            } else {
                normalized
            };
            format!("{}.{}", normalized, market.to_ascii_uppercase())
        }
        Some((code, market)) => format!("{}.{}", code, market.to_ascii_uppercase()),
        None => symbol.to_string(),
    }
}

pub fn default_watchlist() -> Vec<WatchItem> {
    vec![
        WatchItem::new("000001.SH", "上证指数", Market::Cn),
        WatchItem::new("399001.SZ", "深证成指", Market::Cn),
        WatchItem::new("600519.SH", "贵州茅台", Market::Cn),
        WatchItem::new("00700.HK", "腾讯控股", Market::Hk),
        WatchItem::new("09988.HK", "阿里巴巴-W", Market::Hk),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_watchlist_symbols_and_names() {
        let parsed = parse_watchlist("600519.SH:贵州茅台,00700.HK:腾讯控股").unwrap();

        assert_eq!(parsed[0].symbol, "600519.SH");
        assert_eq!(parsed[0].name, "贵州茅台");
        assert_eq!(parsed[0].market, Market::Cn);
        assert_eq!(parsed[1].market, Market::Hk);
    }

    #[test]
    fn keeps_hk_display_symbol_but_normalizes_provider_symbol() {
        let parsed = parse_watchlist("00700.HK:腾讯控股").unwrap();

        assert_eq!(parsed[0].symbol, "00700.HK");
        assert_eq!(parsed[0].provider_symbol, "700.HK");
    }
}
