use std::{collections::BTreeMap, fs, io::Write, sync::Arc};

use chrono::{DateTime, Utc};
use tokio::sync::{Mutex, RwLock, broadcast, watch};

use crate::{
    config::{AppConfig, PersistedWatchItem, WatchItem, normalize_display_symbol},
    error::AppError,
    models::{HealthResponse, Quote, StreamMessage, WatchlistItem},
};

#[derive(Debug)]
struct QuoteStore {
    quotes: BTreeMap<String, Quote>,
    watchlist: Vec<WatchItem>,
    watchlist_revision: u64,
    provider_status: String,
    last_quote_ts: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct AppState {
    config: Arc<AppConfig>,
    store: Arc<RwLock<QuoteStore>>,
    watchlist_mutation_lock: Arc<Mutex<()>>,
    broadcaster: broadcast::Sender<StreamMessage>,
    shutdown_tx: watch::Sender<bool>,
    watchlist_revision_tx: watch::Sender<u64>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let (broadcaster, _) = broadcast::channel(512);
        let (shutdown_tx, _) = watch::channel(false);
        let (watchlist_revision_tx, _) = watch::channel(0);
        let watchlist = config.initial_watchlist.clone();

        Self {
            config: Arc::new(config),
            store: Arc::new(RwLock::new(QuoteStore {
                quotes: BTreeMap::new(),
                watchlist,
                watchlist_revision: 0,
                provider_status: "starting".to_string(),
                last_quote_ts: None,
            })),
            watchlist_mutation_lock: Arc::new(Mutex::new(())),
            broadcaster,
            shutdown_tx,
            watchlist_revision_tx,
        }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamMessage> {
        self.broadcaster.subscribe()
    }

    pub fn shutdown_rx(&self) -> watch::Receiver<bool> {
        self.shutdown_tx.subscribe()
    }

    pub fn watchlist_revision_rx(&self) -> watch::Receiver<u64> {
        self.watchlist_revision_tx.subscribe()
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    pub async fn set_provider_status(&self, status: impl Into<String>) {
        let status = status.into();
        self.store.write().await.provider_status = status.clone();
        let _ = self.broadcaster.send(StreamMessage::Status {
            status,
            server_ts: Utc::now(),
        });
    }

    pub async fn upsert_quote(&self, quote: Quote) {
        {
            let mut store = self.store.write().await;
            if !store
                .watchlist
                .iter()
                .any(|item| item.symbol == quote.symbol)
            {
                return;
            }
            store.last_quote_ts = Some(quote.quote_ts);
            store.quotes.insert(quote.symbol.clone(), quote.clone());
        }

        let _ = self.broadcaster.send(StreamMessage::Quote { quote });
    }

    pub async fn watchlist(&self) -> Vec<WatchItem> {
        self.store.read().await.watchlist.clone()
    }

    pub async fn watchlist_response_items(&self) -> Vec<WatchlistItem> {
        self.watchlist().await.iter().map(Into::into).collect()
    }

    pub async fn upsert_watch_item(
        &self,
        symbol: impl Into<String>,
        name: Option<String>,
    ) -> Result<Vec<WatchlistItem>, AppError> {
        let _guard = self.watchlist_mutation_lock.lock().await;
        let item = WatchItem::from_input(symbol.into(), name).map_err(AppError::Config)?;
        let (changed, items) = {
            let store = self.store.read().await;
            let mut items = store.watchlist.clone();
            match items
                .iter_mut()
                .find(|existing| existing.symbol == item.symbol)
            {
                Some(existing) if *existing == item => (false, items),
                Some(existing) => {
                    *existing = item;
                    items.sort_by(|left, right| left.symbol.cmp(&right.symbol));
                    (true, items)
                }
                None => {
                    items.push(item);
                    items.sort_by(|left, right| left.symbol.cmp(&right.symbol));
                    (true, items)
                }
            }
        };

        if !changed {
            return Ok(items.iter().map(Into::into).collect());
        }

        self.persist_watchlist(&items)?;
        let revision = {
            let mut store = self.store.write().await;
            store.watchlist = items.clone();
            store.watchlist_revision += 1;
            store.watchlist_revision
        };
        self.notify_watchlist_changed(revision);
        Ok(items.iter().map(Into::into).collect())
    }

    pub async fn delete_watch_item(
        &self,
        symbol: &str,
    ) -> Result<(bool, Vec<WatchlistItem>), AppError> {
        let _guard = self.watchlist_mutation_lock.lock().await;
        let symbol = normalize_display_symbol(symbol).map_err(AppError::Config)?;
        let (deleted, items) = {
            let store = self.store.read().await;
            let mut items = store.watchlist.clone();
            let before = items.len();
            items.retain(|item| item.symbol != symbol);
            (items.len() != before, items)
        };

        if deleted {
            self.persist_watchlist(&items)?;
            let revision = {
                let mut store = self.store.write().await;
                store.watchlist = items.clone();
                store.quotes.remove(&symbol);
                store.watchlist_revision += 1;
                store.watchlist_revision
            };
            self.notify_watchlist_changed(revision);
            let _ = self.broadcaster.send(StreamMessage::Snapshot {
                quotes: self.snapshot().await,
            });
        }

        Ok((deleted, items.iter().map(Into::into).collect()))
    }

    pub async fn snapshot(&self) -> Vec<Quote> {
        let now = Utc::now();
        self.store
            .read()
            .await
            .quotes
            .values()
            .cloned()
            .map(|quote| quote.with_freshness(now, self.config.stale_after))
            .collect()
    }

    pub async fn health(&self) -> HealthResponse {
        let store = self.store.read().await;
        HealthResponse {
            status: if store.provider_status == "running" {
                "ok".to_string()
            } else {
                "degraded".to_string()
            },
            provider: self.config.provider.as_str().to_string(),
            provider_status: store.provider_status.clone(),
            quote_count: store.quotes.len(),
            last_quote_ts: store.last_quote_ts,
            server_ts: Utc::now(),
        }
    }

    fn notify_watchlist_changed(&self, revision: u64) {
        let _ = self.watchlist_revision_tx.send(revision);
    }

    fn persist_watchlist(&self, items: &[WatchItem]) -> Result<(), AppError> {
        let Some(path) = self.config.watchlist_file.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|err| {
                AppError::Internal(format!("failed to create {}: {err}", parent.display()))
            })?;
        }

        let payload = items
            .iter()
            .map(PersistedWatchItem::from)
            .collect::<Vec<_>>();
        let data = serde_json::to_string_pretty(&payload)
            .map_err(|err| AppError::Internal(format!("failed to serialize watchlist: {err}")))?;
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("watchlist.json");
        let tmp_path = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
        let mut file = fs::File::create(&tmp_path).map_err(|err| {
            AppError::Internal(format!("failed to create {}: {err}", tmp_path.display()))
        })?;
        file.write_all(data.as_bytes()).map_err(|err| {
            AppError::Internal(format!("failed to write {}: {err}", tmp_path.display()))
        })?;
        file.sync_all().map_err(|err| {
            AppError::Internal(format!("failed to sync {}: {err}", tmp_path.display()))
        })?;
        drop(file);

        fs::rename(&tmp_path, path).map_err(|err| {
            let _ = fs::remove_file(&tmp_path);
            AppError::Internal(format!("failed to replace {}: {err}", path.display()))
        })?;
        if let Some(parent) = path.parent()
            && let Ok(directory) = fs::File::open(parent)
        {
            let _ = directory.sync_all();
        }
        Ok(())
    }
}
