use axum::{
    Json, Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::Deserialize;
use tokio::time::{Duration, MissedTickBehavior, interval, sleep};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};

use crate::{
    config::QuoteProviderKind,
    models::{StreamMessage, WatchlistResponse},
    providers::{QuoteProvider, longbridge::LongbridgeQuoteProvider, mock::MockQuoteProvider},
    state::AppState,
};

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/watchlist", get(watchlist))
        .route("/v1/quotes/stream", get(quotes_stream))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub fn spawn_provider(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut shutdown = state.shutdown_rx();

        loop {
            let provider: Box<dyn QuoteProvider> = match state.config().provider {
                QuoteProviderKind::Mock => {
                    Box::new(MockQuoteProvider::new(state.config().watchlist.clone()))
                }
                QuoteProviderKind::Longbridge => Box::new(LongbridgeQuoteProvider::new(
                    state.config().watchlist.clone(),
                )),
            };

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
            }

            if *shutdown.borrow() || state.config().provider == QuoteProviderKind::Mock {
                return;
            }

            sleep(Duration::from_secs(5)).await;
        }
    })
}

async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::models::HealthResponse>, AuthError> {
    authorize(&state, &headers, None)?;
    Ok(Json(state.health().await))
}

async fn watchlist(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WatchlistResponse>, AuthError> {
    authorize(&state, &headers, None)?;
    Ok(Json(WatchlistResponse {
        items: state.config().watchlist.iter().map(Into::into).collect(),
    }))
}

async fn quotes_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(auth): Query<AuthQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, AuthError> {
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

#[derive(Debug)]
struct AuthError;

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<(), AuthError> {
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
        Err(AuthError)
    }
}
