import type {
  DeleteWatchItemResponse,
  HealthResponse,
  WatchlistResponse,
} from './types'

const TOKEN_STORAGE_KEY = 'deviceToken'

export interface UpsertWatchItemPayload {
  symbol: string
  name: string | null
}

export class ApiError extends Error {
  readonly status: number

  constructor(status: number, statusText: string, detail?: string) {
    super(detail ? `${status} ${statusText}: ${detail}` : `${status} ${statusText}`)
    this.name = 'ApiError'
    this.status = status
  }
}

export function getStoredToken() {
  return localStorage.getItem(TOKEN_STORAGE_KEY) ?? ''
}

export function setStoredToken(token: string) {
  localStorage.setItem(TOKEN_STORAGE_KEY, token)
}

export function fetchHealth(token: string) {
  return apiJson<HealthResponse>('/v1/health', token)
}

export function fetchAdminWatchlist(token: string) {
  return apiJson<WatchlistResponse>('/v1/admin/watchlist', token)
}

export function upsertWatchItem(token: string, payload: UpsertWatchItemPayload) {
  return apiJson<WatchlistResponse>('/v1/admin/watchlist', token, {
    body: JSON.stringify({
      symbol: payload.symbol.trim(),
      name: payload.name?.trim() || null,
    }),
    method: 'POST',
  })
}

export function deleteWatchItem(token: string, symbol: string) {
  return apiJson<DeleteWatchItemResponse>(
    `/v1/admin/watchlist/${encodeURIComponent(symbol)}`,
    token,
    { method: 'DELETE' },
  )
}

async function apiJson<T>(path: string, token: string, init: RequestInit = {}) {
  const headers = new Headers(init.headers)
  headers.set('Accept', 'application/json')
  if (init.body && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json')
  }
  if (token.trim()) {
    headers.set('Authorization', `Bearer ${token.trim()}`)
  }

  const response = await fetch(path, {
    ...init,
    headers,
  })

  if (!response.ok) {
    throw new ApiError(response.status, response.statusText, await readErrorDetail(response))
  }

  return response.json() as Promise<T>
}

async function readErrorDetail(response: Response) {
  try {
    const payload = (await response.json()) as { error?: string }
    return payload.error
  } catch {
    return undefined
  }
}
