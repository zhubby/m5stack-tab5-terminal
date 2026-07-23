# Backend

Rust + Axum service that relays Longbridge or mock quotes to Tab5 devices.

## Configuration

Environment variables:

| Name | Default | Purpose |
| --- | --- | --- |
| `BIND_ADDR` | `127.0.0.1:8080` | HTTP/WebSocket bind address |
| `QUOTE_PROVIDER` | `mock` | `mock` or `longbridge` |
| `WATCHLIST` | built-in demo list | Comma-separated `SYMBOL:Name` entries |
| `WATCHLIST_FILE` | `watchlist.json` | JSON file used for browser/admin watchlist edits |
| `STALE_AFTER_SECS` | `20` | Device stale threshold |
| `MOCK_INTERVAL_MS` | `3000` | Mock update interval |
| `DEVICE_TOKEN` | unset | Required bearer token for admin APIs; optional bearer/query token for device access |

Longbridge mode also requires the SDK environment variables:

| Name |
| --- |
| `LONGBRIDGE_APP_KEY` |
| `LONGBRIDGE_APP_SECRET` |
| `LONGBRIDGE_ACCESS_TOKEN` |
| `LONGBRIDGE_LANGUAGE` |

## Routes

- `GET /v1/health`
- `GET /v1/watchlist`
- `GET /v1/admin/watchlist`
- `POST /v1/admin/watchlist`
- `DELETE /v1/admin/watchlist/{symbol}`
- `WS /v1/quotes/stream`

The browser UI is served at `/`. Admin API calls always require `Authorization: Bearer <token>` with `DEVICE_TOKEN` configured. The WebSocket sends one snapshot immediately after connection, then quote/status/error messages. When `DEVICE_TOKEN` is set, pass `Authorization: Bearer <token>` for HTTP endpoints or `?token=<token>` for constrained WebSocket clients.
