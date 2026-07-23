use std::{collections::BTreeMap, sync::Arc};

use chrono::{DateTime, Utc};
use tokio::sync::{RwLock, broadcast, watch};

use crate::{
    config::AppConfig,
    models::{HealthResponse, Quote, StreamMessage},
};

#[derive(Debug)]
struct QuoteStore {
    quotes: BTreeMap<String, Quote>,
    provider_status: String,
    last_quote_ts: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct AppState {
    config: Arc<AppConfig>,
    store: Arc<RwLock<QuoteStore>>,
    broadcaster: broadcast::Sender<StreamMessage>,
    shutdown_tx: watch::Sender<bool>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let (broadcaster, _) = broadcast::channel(512);
        let (shutdown_tx, _) = watch::channel(false);

        Self {
            config: Arc::new(config),
            store: Arc::new(RwLock::new(QuoteStore {
                quotes: BTreeMap::new(),
                provider_status: "starting".to_string(),
                last_quote_ts: None,
            })),
            broadcaster,
            shutdown_tx,
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
            store.last_quote_ts = Some(quote.quote_ts);
            store.quotes.insert(quote.symbol.clone(), quote.clone());
        }

        let _ = self.broadcaster.send(StreamMessage::Quote { quote });
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
}
