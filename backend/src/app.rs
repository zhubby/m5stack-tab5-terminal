use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, header},
    response::{Html, IntoResponse},
    routing::{delete, get},
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::Deserialize;
use tokio::time::{Duration, MissedTickBehavior, interval, sleep};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};

use crate::{
    config::QuoteProviderKind,
    error::AppError,
    models::{DeleteWatchItemResponse, StreamMessage, UpsertWatchItemRequest, WatchlistResponse},
    providers::{QuoteProvider, longbridge::LongbridgeQuoteProvider, mock::MockQuoteProvider},
    state::AppState,
};

const ADMIN_HTML: &str = include_str!("admin.html");

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(admin_page))
        .route("/admin", get(admin_page))
        .route("/v1/health", get(health))
        .route("/v1/watchlist", get(watchlist))
        .route(
            "/v1/admin/watchlist",
            get(admin_watchlist).post(admin_upsert_watch_item),
        )
        .route(
            "/v1/admin/watchlist/{symbol}",
            delete(admin_delete_watch_item),
        )
        .route("/v1/quotes/stream", get(quotes_stream))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub fn spawn_provider(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut shutdown = state.shutdown_rx();
        let mut watchlist_revision = state.watchlist_revision_rx();

        loop {
            let watchlist = state.watchlist().await;
            let provider: Box<dyn QuoteProvider> = match state.config().provider {
                QuoteProviderKind::Mock => Box::new(MockQuoteProvider::new(watchlist)),
                QuoteProviderKind::Longbridge => Box::new(LongbridgeQuoteProvider::new(watchlist)),
            };

            let mut restart_for_watchlist = false;
            tokio::select! {
                result = provider.run(state.clone()) => {
                    if let Err(err) = result {
                        error!(%err, "quote provider exited; retrying");
                        state.set_provider_status(format!("degraded: {err}")).await;
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        return;
                    }
                }
                changed = watchlist_revision.changed() => {
                    if changed.is_ok() {
                        restart_for_watchlist = true;
                    }
                }
            }

            if *shutdown.borrow() {
                return;
            }

            if state.config().provider == QuoteProviderKind::Mock && !restart_for_watchlist {
                return;
            }

            if !restart_for_watchlist {
                sleep(Duration::from_secs(5)).await;
            }
        }
    })
}

async fn admin_page() -> Html<&'static str> {
    Html(ADMIN_HTML)
}

async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::models::HealthResponse>, AppError> {
    authorize(&state, &headers, None)?;
    Ok(Json(state.health().await))
}

async fn watchlist(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WatchlistResponse>, AppError> {
    authorize(&state, &headers, None)?;
    Ok(Json(watchlist_response(&state).await))
}

async fn admin_watchlist(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WatchlistResponse>, AppError> {
    authorize_admin(&state, &headers)?;
    Ok(Json(watchlist_response(&state).await))
}

async fn admin_upsert_watch_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpsertWatchItemRequest>,
) -> Result<Json<WatchlistResponse>, AppError> {
    authorize_admin(&state, &headers)?;
    let items = state
        .upsert_watch_item(request.symbol, request.name)
        .await?;
    Ok(Json(WatchlistResponse { items }))
}

async fn admin_delete_watch_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(symbol): Path<String>,
) -> Result<Json<DeleteWatchItemResponse>, AppError> {
    authorize_admin(&state, &headers)?;
    let (deleted, items) = state.delete_watch_item(&symbol).await?;
    Ok(Json(DeleteWatchItemResponse { deleted, items }))
}

async fn quotes_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(auth): Query<AuthQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, AppError> {
    authorize(&state, &headers, auth.token.as_deref())?;
    Ok(ws.on_upgrade(move |socket| handle_socket(state, socket)))
}

async fn handle_socket(state: AppState, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let mut quote_rx = state.subscribe();
    let snapshot = StreamMessage::Snapshot {
        quotes: state.snapshot().await,
    };

    if send_json(&mut sender, &snapshot).await.is_err() {
        return;
    }

    let mut stale_tick = interval(Duration::from_secs(1));
    stale_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            inbound = receiver.next() => {
                match inbound {
                    Some(Ok(Message::Ping(payload))) => {
                        if sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        info!(%err, "websocket client read failed");
                        break;
                    }
                }
            }
            outbound = quote_rx.recv() => {
                match outbound {
                    Ok(message) => {
                        if send_json(&mut sender, &message).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        let message = StreamMessage::Error {
                            message: format!("client lagged and skipped {skipped} updates"),
                            server_ts: chrono::Utc::now(),
                        };
                        if send_json(&mut sender, &message).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = stale_tick.tick() => {
                let message = StreamMessage::Snapshot {
                    quotes: state.snapshot().await,
                };
                if send_json(&mut sender, &message).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn send_json(
    sender: &mut SplitSink<WebSocket, Message>,
    message: &StreamMessage,
) -> Result<(), ()> {
    match serde_json::to_string(message) {
        Ok(json) => sender
            .send(Message::Text(json.into()))
            .await
            .map_err(|_| ()),
        Err(err) => {
            error!(%err, "failed to serialize stream message");
            let fallback = StreamMessage::Error {
                message: err.to_string(),
                server_ts: chrono::Utc::now(),
            };
            if let Ok(json) = serde_json::to_string(&fallback) {
                let _ = sender.send(Message::Text(json.into())).await;
            }
            Err(())
        }
    }
}

#[derive(Debug, Deserialize)]
struct AuthQuery {
    token: Option<String>,
}

fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<(), AppError> {
    let Some(expected) = state.config().device_token.as_deref() else {
        return Ok(());
    };

    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    if bearer == Some(expected) || query_token == Some(expected) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

fn authorize_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let Some(expected) = state.config().device_token.as_deref() else {
        return Err(AppError::Unauthorized);
    };

    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    if bearer == Some(expected) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

async fn watchlist_response(state: &AppState) -> WatchlistResponse {
    WatchlistResponse {
        items: state.watchlist_response_items().await,
    }
}
