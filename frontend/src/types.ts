export type Market = 'cn' | 'hk'

export type QuoteStatus =
  | 'normal'
  | 'stale'
  | 'offline'
  | 'market_closed'
  | 'suspended'

export interface Quote {
  symbol: string
  name: string
  market: Market
  last: number
  change: number
  change_pct: number
  volume: number
  turnover: number
  trade_status: string
  status: QuoteStatus
  quote_ts: string
  server_ts: string
  stale: boolean
  stale_after_ms: number
}

export interface WatchlistItem {
  symbol: string
  name: string
  market: Market
}

export interface WatchlistResponse {
  items: WatchlistItem[]
}

export interface DeleteWatchItemResponse {
  deleted: boolean
  items: WatchlistItem[]
}

export interface HealthResponse {
  status: string
  provider: string
  provider_status: string
  quote_count: number
  last_quote_ts: string | null
  server_ts: string
}

export type StreamMessage =
  | { type: 'snapshot'; quotes: Quote[] }
  | { type: 'quote'; quote: Quote }
  | { type: 'status'; status: string; server_ts: string }
  | { type: 'error'; message: string; server_ts: string }
