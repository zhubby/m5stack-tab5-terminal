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
| `FRONTEND_DIST_DIR` | `frontend/dist` | Vite build output served for `/`, `/admin`, and static assets |
| `STALE_AFTER_SECS` | `20` | Device stale threshold |
| `DETAIL_CACHE_TTL_SECS` | `30` | Per-symbol intraday detail cache TTL |
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
- `GET /v1/quotes/{symbol}/detail`
- `WS /v1/quotes/stream`

The browser UI is served at `/` and `/admin` after `frontend/dist` is built. Missing frontend build output does not block API startup; page routes return a clear not-built response. Admin API calls always require `Authorization: Bearer <token>` with `DEVICE_TOKEN` configured. The WebSocket sends one snapshot immediately after connection, then quote/status/error messages. Tab5 can request the selected card detail over the same socket with `{"type":"detail_request","request_id":1,"symbol":"600519.SH"}` and receives `detail` or `detail_error`. When `DEVICE_TOKEN` is set, pass `Authorization: Bearer <token>` for HTTP endpoints or `?token=<token>` for constrained WebSocket clients.

## Browser UI development

Run the backend in one terminal:

```bash
cargo run -p tab5-stock-backend
```

Run the React admin UI in another:

```bash
cd frontend
npm install
VITE_BACKEND_ORIGIN=http://127.0.0.1:8080 npm run dev
```

For production-style local serving, run `npm run build` in `frontend/`, then start the backend and open `http://localhost:8080/admin`.
