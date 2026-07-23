# M5Stack Tab5 stock terminal

Near-real-time A-share/HK stock monitor for M5Stack Tab5.

## Architecture

- `backend/`: Rust + Axum quote relay.
- `frontend/`: Vite + React + TypeScript browser admin UI.
- `firmware/`: ESP-IDF C++ firmware for M5Stack Tab5 using LVGL.
- Quote source: mock by default, Longbridge OpenAPI when configured.
- Device protocol: WebSocket JSON stream from `/v1/quotes/stream`.

The Tab5 firmware never stores Longbridge credentials. It only connects to the backend.

## Backend quick start

```bash
cargo run -p tab5-stock-backend
```

Default mode is mock quotes bound to `127.0.0.1:8080`. API routes stay available even when the frontend has not been built. Useful endpoints:

```text
WS  ws://localhost:8080/v1/quotes/stream
GET http://localhost:8080/v1/health
GET http://localhost:8080/v1/watchlist
GET http://localhost:8080/v1/admin/watchlist
POST http://localhost:8080/v1/admin/watchlist
DELETE http://localhost:8080/v1/admin/watchlist/<symbol>
```

Admin API calls require `DEVICE_TOKEN`; add/delete operations update `WATCHLIST_FILE` and restart the active quote provider subscription.

## Frontend quick start

For browser UI development, run the backend and Vite separately:

```bash
cd frontend
npm install
VITE_BACKEND_ORIGIN=http://127.0.0.1:8080 npm run dev
```

Open `http://localhost:5173/admin`. The Vite dev server proxies `/v1/*` and WebSocket traffic to the backend. Enter the same `DEVICE_TOKEN` used by the backend.

For single-service production deployment:

```bash
cd frontend
npm run build
cd ..
cargo run -p tab5-stock-backend
```

Axum serves `/`, `/admin`, and static assets from `FRONTEND_DIST_DIR` which defaults to `frontend/dist`. If `frontend/dist/index.html` is missing, page routes return an explicit frontend-not-built response while `/v1/*` continues to hit the backend API.

Use Longbridge by setting:

```bash
QUOTE_PROVIDER=longbridge
LONGBRIDGE_APP_KEY=...
LONGBRIDGE_APP_SECRET=...
LONGBRIDGE_ACCESS_TOKEN=...
LONGBRIDGE_LANGUAGE=zh-CN
```

Set `DEVICE_TOKEN` for browser/admin watchlist management. Also set it before binding the backend to `0.0.0.0` for LAN access, then pass it as a bearer token or WebSocket query parameter:

```text
BIND_ADDR=0.0.0.0:8080
DEVICE_TOKEN=<random-token>
ws://<backend-host>:8080/v1/quotes/stream?token=<DEVICE_TOKEN>
Authorization: Bearer <DEVICE_TOKEN>
```

## Firmware quick start

Install ESP-IDF with ESP32-P4 support, then:

```bash
cd firmware
idf.py set-target esp32p4
idf.py menuconfig
idf.py build
idf.py flash monitor
```

Leave Wi-Fi SSID empty to run local mock mode on the device. Configure Wi-Fi and `CONFIG_TAB5_STOCK_BACKEND_URI` to connect to the backend WebSocket stream.

## JSON stream

The backend sends:

```json
{"type":"snapshot","quotes":[]}
{"type":"quote","quote":{"symbol":"600519.SH","name":"贵州茅台","market":"cn","last":1682.65,"change":9.2,"change_pct":0.55,"volume":2600000,"turnover":4374000000.0,"trade_status":"normal","status":"normal","quote_ts":"2026-07-23T09:30:03Z","server_ts":"2026-07-23T09:30:04Z","stale":false,"stale_after_ms":20000}}
{"type":"status","status":"running","server_ts":"2026-07-23T00:00:00Z"}
{"type":"error","message":"client lagged and skipped 12 updates","server_ts":"2026-07-23T00:00:00Z"}
```

The full quote DTO includes:

- `symbol`
- `name`
- `market`
- `last`
- `change`
- `change_pct`
- `volume`
- `turnover`
- `trade_status`
- `status`
- `quote_ts`
- `server_ts`
- `stale`
- `stale_after_ms`

## Validation

Backend:

```bash
cargo test -p tab5-stock-backend
```

Frontend:

```bash
cd frontend
npm test
npm run lint
npm run build
```

Firmware requires an ESP-IDF environment and Tab5 hardware for final validation.
