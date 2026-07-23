use async_trait::async_trait;

use crate::{error::AppError, state::AppState};

pub mod longbridge;
pub mod mock;

#[async_trait]
pub trait QuoteProvider: Send + Sync + 'static {
    async fn run(self: Box<Self>, state: AppState) -> Result<(), AppError>;
}
