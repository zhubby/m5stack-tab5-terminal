pub mod app;
pub mod config;
pub mod error;
pub mod models;
pub mod providers;
pub mod state;

pub use app::{app, spawn_provider};
pub use config::{AppConfig, QuoteProviderKind, WatchItem};
pub use models::{HealthResponse, Market, Quote, QuoteStatus, StreamMessage, WatchlistResponse};
pub use state::AppState;
