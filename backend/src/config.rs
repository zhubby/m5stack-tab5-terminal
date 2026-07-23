use std::{
    env, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};

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
    pub initial_watchlist: Vec<WatchItem>,
    pub watchlist_file: Option<PathBuf>,
    pub frontend_dist_dir: PathBuf,
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
        let symbol = symbol.into().trim().to_ascii_uppercase();
        Self {
            provider_symbol: normalize_provider_symbol(&symbol),
            symbol,
            name: name.into(),
            market,
        }
    }

    pub fn from_input(symbol: impl Into<String>, name: Option<String>) -> Result<Self, String> {
        let symbol = normalize_display_symbol(&symbol.into())?;
        let name = name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| symbol.clone());
        Ok(Self::new(symbol.clone(), name, Market::infer(&symbol)))
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
        let watchlist_file = env::var("WATCHLIST_FILE")
            .map(|value| value.trim().to_string())
            .ok()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from("watchlist.json")));
        let frontend_dist_dir = env::var("FRONTEND_DIST_DIR")
            .map(|value| value.trim().to_string())
            .ok()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("frontend/dist"));
        let env_watchlist = parse_watchlist(&env::var("WATCHLIST").unwrap_or_default())?;
        let watchlist = load_or_default_watchlist(watchlist_file.as_deref(), env_watchlist)?;
        let device_token = env::var("DEVICE_TOKEN")
            .ok()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());

        Ok(Self {
            bind_addr,
            provider,
            initial_watchlist: watchlist,
            watchlist_file,
            frontend_dist_dir,
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

            WatchItem::from_input(symbol, Some(name.to_string()))
        })
        .collect()
}

fn load_or_default_watchlist(
    watchlist_file: Option<&Path>,
    env_watchlist: Vec<WatchItem>,
) -> Result<Vec<WatchItem>, String> {
    if let Some(path) = watchlist_file
        && let Some(items) = load_watchlist_file(path)?
    {
        return Ok(items);
    }

    if env_watchlist.is_empty() {
        Ok(default_watchlist())
    } else {
        Ok(env_watchlist)
    }
}

fn load_watchlist_file(path: &Path) -> Result<Option<Vec<WatchItem>>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let data = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let items = serde_json::from_str::<Vec<PersistedWatchItem>>(&data)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?
        .into_iter()
        .map(|item| WatchItem::from_input(item.symbol, Some(item.name)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(items))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedWatchItem {
    pub symbol: String,
    pub name: String,
}

impl From<&WatchItem> for PersistedWatchItem {
    fn from(item: &WatchItem) -> Self {
        Self {
            symbol: item.symbol.clone(),
            name: item.name.clone(),
        }
    }
}

pub fn normalize_display_symbol(symbol: &str) -> Result<String, String> {
    let symbol = symbol.trim().to_ascii_uppercase();
    if symbol.is_empty() {
        return Err("symbol is required".to_string());
    }
    if !symbol.contains('.') {
        return Err(
            "symbol must include market suffix, for example 600519.SH or 00700.HK".to_string(),
        );
    }
    Ok(symbol)
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
    use std::{
        env, fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn parses_watchlist_symbols_and_names() {
        let parsed = parse_watchlist("600519.sh:贵州茅台,00700.hk:腾讯控股").unwrap();

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

    #[test]
    fn normalizes_persisted_watchlist_symbols() {
        let path = temp_watchlist_path("normalized");
        fs::write(&path, r#"[{"symbol":"00700.hk","name":"腾讯控股"}]"#).unwrap();

        let resolved = load_or_default_watchlist(Some(path.as_path()), Vec::new()).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(resolved[0].symbol, "00700.HK");
        assert_eq!(resolved[0].provider_symbol, "700.HK");
        assert_eq!(resolved[0].market, Market::Hk);
    }

    #[test]
    fn persisted_empty_watchlist_stays_empty() {
        let path = temp_watchlist_path("empty");
        fs::write(&path, "[]").unwrap();

        let resolved = load_or_default_watchlist(Some(path.as_path()), Vec::new()).unwrap();

        fs::remove_file(path).unwrap();
        assert!(resolved.is_empty());
    }

    #[test]
    fn missing_watchlist_file_falls_back_to_defaults() {
        let path = temp_watchlist_path("missing");
        let _ = fs::remove_file(&path);

        let resolved = load_or_default_watchlist(Some(path.as_path()), Vec::new()).unwrap();

        assert_eq!(resolved, default_watchlist());
    }

    fn temp_watchlist_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("tab5-stock-{label}-{unique}.json"))
    }
}
