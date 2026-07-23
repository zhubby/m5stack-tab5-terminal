use async_trait::async_trait;

use crate::{
    config::{AppConfig, QuoteProviderKind, WatchItem},
    error::AppError,
    models::{IntradayPoint, Quote},
    state::AppState,
};

pub mod longbridge;
pub mod mock;

#[async_trait]
pub trait QuoteProvider: Send + Sync + 'static {
    async fn run(self: Box<Self>, state: AppState) -> Result<(), AppError>;
}

pub async fn fetch_intraday(
    config: &AppConfig,
    item: &WatchItem,
    quote: &Quote,
) -> Result<Vec<IntradayPoint>, AppError> {
    match config.provider {
        QuoteProviderKind::Mock => Ok(mock::mock_intraday(item, quote)),
        QuoteProviderKind::Longbridge => longbridge::fetch_intraday(item).await,
    }
}
