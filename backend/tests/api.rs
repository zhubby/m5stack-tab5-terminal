use std::{
    fs,
    net::SocketAddr,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::http::StatusCode;
use chrono::Utc;
use futures_util::StreamExt;
use tab5_stock_backend::{
    AppConfig, AppState, Market, QuoteProviderKind, StreamMessage, WatchItem, WatchlistResponse,
    app, providers::mock::mock_quote, spawn_provider,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;

fn test_config() -> AppConfig {
    AppConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        provider: QuoteProviderKind::Mock,
        initial_watchlist: vec![
            WatchItem::new("600519.SH", "贵州茅台", Market::Cn),
            WatchItem::new("00700.HK", "腾讯控股", Market::Hk),
        ],
        watchlist_file: None,
        frontend_dist_dir: temp_frontend_dist_path("missing"),
        stale_after: std::time::Duration::from_secs(20),
        mock_interval: std::time::Duration::from_millis(25),
        device_token: None,
    }
}

#[tokio::test]
async fn snapshot_marks_old_quotes_stale() {
    let mut config = test_config();
    config.stale_after = std::time::Duration::from_secs(1);
    let state = AppState::new(config);
    let item = state.watchlist().await[0].clone();
    let mut quote = mock_quote(&item, 0, 1, state.config().stale_after.as_millis() as u64);
    quote.server_ts = Utc::now() - chrono::Duration::seconds(5);

    state.upsert_quote(quote).await;
    let snapshot = state.snapshot().await;

    assert_eq!(snapshot.len(), 1);
    assert!(snapshot[0].stale);
}

#[tokio::test]
async fn device_token_rejects_unauthorized_requests() {
    let mut config = test_config();
    config.device_token = Some("secret".to_string());
    let state = AppState::new(config);
    let app = app(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/watchlist")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    assert!(body.is_empty());
}

#[tokio::test]
async fn device_token_accepts_bearer_header() {
    let mut config = test_config();
    config.device_token = Some("secret".to_string());
    let state = AppState::new(config);
    let app = app(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/watchlist")
                .header(axum::http::header::AUTHORIZATION, "Bearer secret")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn provider_keeps_running_after_request_state_clone_drops() {
    let state = AppState::new(test_config());
    let provider_handle = spawn_provider(state.clone());
    let app = app(state.clone());

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    timeout(Duration::from_secs(2), async {
        loop {
            if !state.snapshot().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("provider stopped after request state clone dropped");

    state.shutdown();
    provider_handle.abort();
}

#[tokio::test]
async fn websocket_accepts_device_token_query() {
    let mut config = test_config();
    config.device_token = Some("secret".to_string());
    let state = AppState::new(config);
    let (addr, server_shutdown, server_handle) = serve_test_app(state.clone()).await;

    let (mut socket, _) = timeout(
        Duration::from_secs(2),
        connect_async(format!("ws://{addr}/v1/quotes/stream?token=secret")),
    )
    .await
    .expect("websocket connect timed out")
    .unwrap();

    let first = timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("snapshot timed out")
        .unwrap()
        .unwrap();

    assert!(matches!(first, Message::Text(_)));
    drop(socket);
    let _ = server_shutdown.send(());
    server_handle.abort();
}

#[tokio::test]
async fn health_reports_provider_and_quote_state() {
    let state = AppState::new(test_config());
    state.set_provider_status("running").await;
    let app = app(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["provider"], "mock");
}

#[tokio::test]
async fn watchlist_returns_cn_and_hk_symbols() {
    let state = AppState::new(test_config());
    let app = app(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/watchlist")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: WatchlistResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload.items.len(), 2);
    assert_eq!(payload.items[0].symbol, "600519.SH");
    assert_eq!(payload.items[1].market, Market::Hk);
}

#[tokio::test]
async fn admin_can_add_and_delete_watch_items() {
    let mut config = test_config();
    config.device_token = Some("secret".to_string());
    let state = AppState::new(config);
    let app = app(state.clone());

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/admin/watchlist")
                .header(axum::http::header::AUTHORIZATION, "Bearer secret")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    r#"{"symbol":"09988.HK","name":"阿里巴巴-W"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: WatchlistResponse = serde_json::from_slice(&body).unwrap();
    assert!(payload.items.iter().any(|item| item.symbol == "09988.HK"));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/v1/admin/watchlist/09988.HK")
                .header(axum::http::header::AUTHORIZATION, "Bearer secret")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["deleted"], true);
    assert!(
        !payload["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["symbol"] == "09988.HK")
    );
}

#[tokio::test]
async fn admin_mutations_require_device_token_even_when_device_routes_are_open() {
    let state = AppState::new(test_config());
    let app = app(state);

    let device_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/watchlist")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(device_response.status(), StatusCode::OK);

    let admin_response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/admin/watchlist")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    r#"{"symbol":"09988.HK","name":"阿里巴巴-W"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_persists_watchlist_edits_to_file() {
    let path = temp_watchlist_path("persist");
    let mut config = test_config();
    config.device_token = Some("secret".to_string());
    config.watchlist_file = Some(path.clone());
    let state = AppState::new(config);
    let app = app(state);

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/admin/watchlist")
                .header(axum::http::header::AUTHORIZATION, "Bearer secret")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    r#"{"symbol":"000002.SZ","name":"万科A"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let persisted = fs::read_to_string(&path).unwrap();
    assert!(persisted.contains("\"symbol\": \"000002.SZ\""));
    assert!(persisted.contains("\"name\": \"万科A\""));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/v1/admin/watchlist/000002.SZ")
                .header(axum::http::header::AUTHORIZATION, "Bearer secret")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let persisted = fs::read_to_string(&path).unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_str(&persisted).unwrap();
    assert!(!items.iter().any(|item| item["symbol"] == "000002.SZ"));
    fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn failed_watchlist_persistence_does_not_commit_runtime_state() {
    let path = temp_watchlist_path("unwritable-target");
    fs::create_dir(&path).unwrap();
    let mut config = test_config();
    config.watchlist_file = Some(path.clone());
    let state = AppState::new(config);
    let before = state.watchlist_response_items().await;

    let result = state
        .upsert_watch_item("09988.HK", Some("阿里巴巴-W".to_string()))
        .await;

    assert!(result.is_err());
    assert_eq!(state.watchlist_response_items().await, before);
    fs::remove_dir(path).unwrap();
}

#[tokio::test]
async fn provider_restarts_after_watchlist_changes() {
    let mut config = test_config();
    config.mock_interval = std::time::Duration::from_millis(10);
    let state = AppState::new(config);
    let provider_handle = spawn_provider(state.clone());

    wait_for_symbol(&state, "600519.SH").await;
    state
        .upsert_watch_item("09988.HK", Some("阿里巴巴-W".to_string()))
        .await
        .unwrap();
    wait_for_symbol(&state, "09988.HK").await;

    state.shutdown();
    provider_handle.abort();
}

#[tokio::test]
async fn quotes_for_deleted_symbols_are_ignored() {
    let state = AppState::new(test_config());
    let deleted_item = state.watchlist().await[0].clone();
    let quote = mock_quote(
        &deleted_item,
        0,
        1,
        state.config().stale_after.as_millis() as u64,
    );
    state.upsert_quote(quote.clone()).await;
    assert!(
        state
            .snapshot()
            .await
            .iter()
            .any(|item| item.symbol == deleted_item.symbol)
    );

    state.delete_watch_item(&deleted_item.symbol).await.unwrap();
    state.upsert_quote(quote).await;

    assert!(
        !state
            .snapshot()
            .await
            .iter()
            .any(|item| item.symbol == deleted_item.symbol)
    );
}

#[tokio::test]
async fn admin_rejects_invalid_symbols() {
    let mut config = test_config();
    config.device_token = Some("secret".to_string());
    let state = AppState::new(config);
    let app = app(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/admin/watchlist")
                .header(axum::http::header::AUTHORIZATION, "Bearer secret")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    r#"{"symbol":"600519","name":"贵州茅台"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn frontend_dist_serves_root_and_admin_fallback() {
    let dist = temp_frontend_dist_path("dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(
        dist.join("index.html"),
        r#"<!doctype html><html><body><div id="root">React Admin</div></body></html>"#,
    )
    .unwrap();

    let mut config = test_config();
    config.frontend_dist_dir = dist.clone();
    let state = AppState::new(config);
    let app = app(state);

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("React Admin"));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/admin")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("React Admin"));
    fs::remove_dir_all(dist).unwrap();
}

#[tokio::test]
async fn api_routes_are_not_intercepted_by_frontend_fallback() {
    let dist = temp_frontend_dist_path("api-fallback");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("index.html"), "React Admin").unwrap();

    let mut config = test_config();
    config.frontend_dist_dir = dist.clone();
    let state = AppState::new(config);
    let app = app(state);

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/not-found")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let body = std::str::from_utf8(&body).unwrap();
    assert!(!body.contains("React Admin"));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let body = std::str::from_utf8(&body).unwrap();
    assert!(!body.contains("React Admin"));
    fs::remove_dir_all(dist).unwrap();
}

#[tokio::test]
async fn missing_frontend_dist_keeps_api_available() {
    let mut config = test_config();
    config.frontend_dist_dir = temp_frontend_dist_path("missing-dist");
    let state = AppState::new(config);
    let app = app(state);

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/admin")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let body = std::str::from_utf8(&body).unwrap();
    assert!(body.contains("frontend dist is not built"));
}

#[tokio::test]
async fn websocket_sends_snapshot_and_quote_updates() {
    let state = AppState::new(test_config());
    let (addr, server_shutdown, server_handle) = serve_test_app(state.clone()).await;

    let (mut socket, _) = timeout(
        Duration::from_secs(2),
        connect_async(format!("ws://{addr}/v1/quotes/stream")),
    )
    .await
    .expect("websocket connect timed out")
    .unwrap();

    let first = timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("snapshot timed out")
        .unwrap()
        .unwrap();
    let Message::Text(snapshot) = first else {
        panic!("expected text snapshot");
    };
    let snapshot: StreamMessage = serde_json::from_str(&snapshot).unwrap();
    assert!(matches!(snapshot, StreamMessage::Snapshot { .. }));

    let mut saw_quote = false;
    let item = state.watchlist().await[0].clone();
    let quote = mock_quote(&item, 0, 1, state.config().stale_after.as_millis() as u64);
    state.upsert_quote(quote).await;

    for _ in 0..10 {
        let next = timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("quote update timed out")
            .unwrap()
            .unwrap();
        if let Message::Text(text) = next {
            let message: StreamMessage = serde_json::from_str(&text).unwrap();
            if let StreamMessage::Quote { quote } = message {
                assert!(quote.symbol == "600519.SH" || quote.symbol == "00700.HK");
                saw_quote = true;
                break;
            }
        }
    }

    drop(socket);
    state.shutdown();
    let _ = server_shutdown.send(());
    server_handle.abort();
    assert!(saw_quote, "expected at least one quote update");
}

#[tokio::test]
async fn quote_message_json_has_firmware_contract_fields() {
    let state = AppState::new(test_config());
    let item = state.watchlist().await[0].clone();
    let quote = mock_quote(&item, 0, 1, state.config().stale_after.as_millis() as u64);
    let message = StreamMessage::Quote { quote };
    let json = serde_json::to_value(&message).unwrap();
    let quote = &json["quote"];

    assert_eq!(json["type"], "quote");
    assert_eq!(quote["symbol"], "600519.SH");
    assert_eq!(quote["market"], "cn");
    assert_eq!(quote["status"], "normal");
    assert!(quote.get("last").unwrap().is_number());
    assert!(quote.get("change_pct").unwrap().is_number());
    assert!(quote.get("quote_ts").unwrap().is_string());
    assert!(quote.get("server_ts").unwrap().is_string());
    assert!(quote.get("stale_after_ms").unwrap().is_number());
}

async fn wait_for_symbol(state: &AppState, symbol: &str) {
    timeout(Duration::from_secs(2), async {
        loop {
            if state
                .snapshot()
                .await
                .iter()
                .any(|quote| quote.symbol == symbol)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {symbol}"));
}

fn temp_watchlist_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("tab5-stock-api-{label}-{unique}.json"))
}

fn temp_frontend_dist_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("tab5-stock-frontend-{label}-{unique}"))
}

async fn serve_test_app(
    state: AppState,
) -> (SocketAddr, oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app(state))
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });
    (addr, shutdown_tx, handle)
}
