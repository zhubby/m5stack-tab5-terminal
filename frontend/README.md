# Tab5 Stock Admin Frontend

Vite + React + TypeScript admin UI for managing the backend watchlist.

## Scripts

```bash
npm run dev
npm run build
npm test
npm run lint
npm run preview
```

`VITE_BACKEND_ORIGIN` controls the Vite dev proxy target and defaults to `http://127.0.0.1:8080`.

## Development

Start the Rust backend first, then run:

```bash
VITE_BACKEND_ORIGIN=http://127.0.0.1:8080 npm run dev
```

Open `http://localhost:5173/admin` and enter the backend `DEVICE_TOKEN` for add/delete operations.
