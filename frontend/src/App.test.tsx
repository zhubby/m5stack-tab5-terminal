import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App'
import type { Quote, WatchlistItem } from './types'

const health = {
  last_quote_ts: null,
  provider: 'mock',
  provider_status: 'running',
  quote_count: 0,
  server_ts: '2026-07-23T09:30:00Z',
  status: 'ok',
}

const initialWatchlist: WatchlistItem[] = [
  { market: 'cn', name: '贵州茅台', symbol: '600519.SH' },
  { market: 'hk', name: '腾讯控股', symbol: '00700.HK' },
]

const maotaiQuote: Quote = {
  change: 9.2,
  change_pct: 0.55,
  last: 1682.65,
  market: 'cn',
  name: '贵州茅台',
  quote_ts: '2026-07-23T09:30:03Z',
  server_ts: '2026-07-23T09:30:04Z',
  stale: false,
  stale_after_ms: 20000,
  status: 'normal',
  symbol: '600519.SH',
  trade_status: 'normal',
  turnover: 4374000000,
  volume: 2600000,
}

class FakeWebSocket extends EventTarget {
  static instances: FakeWebSocket[] = []

  readonly url: string

  constructor(url: string) {
    super()
    this.url = url
    FakeWebSocket.instances.push(this)
  }

  close() {
    this.dispatchEvent(new Event('close'))
  }

  open() {
    this.dispatchEvent(new Event('open'))
  }

  receive(data: unknown) {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(data) }))
  }
}

describe('App', () => {
  beforeEach(() => {
    localStorage.clear()
    FakeWebSocket.instances = []
    vi.stubGlobal('WebSocket', FakeWebSocket)
    vi.stubGlobal('fetch', vi.fn(defaultFetch))
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('loads health and watchlist with the stored token', async () => {
    localStorage.setItem('deviceToken', 'secret')

    render(<App />)

    expect(await screen.findByText('贵州茅台')).toBeInTheDocument()
    expect(screen.getByTestId('后端-status')).toHaveTextContent('ok')
    expect(fetchCallsFor('/v1/admin/watchlist')[0].headers.get('Authorization')).toBe(
      'Bearer secret',
    )
    expect(FakeWebSocket.instances[0].url).toContain('token=secret')
  })

  it('stores token input and sends it on admin mutations', async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByText('贵州茅台')

    await user.type(screen.getByLabelText('Token'), 'secret')
    await user.type(screen.getByLabelText('代码'), '09988.HK')
    await user.type(screen.getByLabelText('名称'), '阿里巴巴-W')
    await user.click(screen.getByRole('button', { name: '添加' }))

    await screen.findByText('阿里巴巴-W')
    expect(localStorage.getItem('deviceToken')).toBe('secret')
    const postCall = fetchCallsFor('/v1/admin/watchlist').find((call) => call.method === 'POST')
    expect(postCall?.headers.get('Authorization')).toBe('Bearer secret')
  })

  it('removes deleted watch items and cached quotes', async () => {
    localStorage.setItem('deviceToken', 'secret')
    const user = userEvent.setup()
    render(<App />)
    await screen.findByText('贵州茅台')
    act(() => {
      FakeWebSocket.instances[0].receive({ quotes: [maotaiQuote], type: 'snapshot' })
    })
    expect(await screen.findByText('1682.65')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '删除 600519.SH' }))

    await waitFor(() => expect(screen.queryByText('600519.SH')).not.toBeInTheDocument())
    expect(screen.queryByText('1682.65')).not.toBeInTheDocument()
    expect(fetchCallsFor('/v1/admin/watchlist/600519.SH')[0].headers.get('Authorization')).toBe(
      'Bearer secret',
    )
  })

  it('renders websocket snapshot, quote, status, and error messages', async () => {
    localStorage.setItem('deviceToken', 'secret')
    render(<App />)
    await screen.findByText('贵州茅台')

    const socket = FakeWebSocket.instances[0]
    act(() => {
      socket.open()
      socket.receive({ quotes: [maotaiQuote], type: 'snapshot' })
    })
    expect(await screen.findByText('1682.65')).toBeInTheDocument()
    expect(screen.getByText('+0.55%')).toHaveClass('up')

    act(() => {
      socket.receive({
        quote: { ...maotaiQuote, change: -3.1, change_pct: -0.18, last: 1670.5 },
        type: 'quote',
      })
    })
    expect(await screen.findByText('1670.50')).toHaveClass('down')

    act(() => {
      socket.receive({ server_ts: '2026-07-23T09:30:05Z', status: 'running', type: 'status' })
    })
    await waitFor(() => expect(screen.getByTestId('行情-status')).toHaveTextContent('running'))

    act(() => {
      socket.receive({ message: 'client lagged', server_ts: '2026-07-23T09:30:06Z', type: 'error' })
    })
    expect(screen.getByRole('status')).toHaveTextContent('client lagged')
  })

  it('shows websocket transport errors and close state', async () => {
    localStorage.setItem('deviceToken', 'secret')

    render(<App />)
    await screen.findByText('贵州茅台')

    const socket = FakeWebSocket.instances[0]
    act(() => {
      socket.dispatchEvent(new Event('error'))
    })
    expect(screen.getByRole('status')).toHaveTextContent('行情连接错误')

    act(() => {
      socket.close()
    })
    await waitFor(() => expect(screen.getByTestId('行情-status')).toHaveTextContent('closed'))
  })

  it('shows clear API errors', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        if (String(input).includes('/v1/admin/watchlist')) {
          return jsonResponse({ error: 'unauthorized' }, { status: 401, statusText: 'Unauthorized' })
        }
        return jsonResponse(health)
      }),
    )

    render(<App />)

    expect(await screen.findByRole('status')).toHaveTextContent('401 Unauthorized: unauthorized')
  })

  it('shows network errors without starting the quote stream', async () => {
    localStorage.setItem('deviceToken', 'secret')
    vi.stubGlobal('fetch', vi.fn(() => Promise.reject(new Error('network down'))))

    render(<App />)

    expect(await screen.findByRole('status')).toHaveTextContent('network down')
    expect(FakeWebSocket.instances).toHaveLength(0)
  })
})

function defaultFetch(input: RequestInfo | URL, init?: RequestInit) {
  const url = String(input)
  const method = init?.method ?? 'GET'

  if (url.endsWith('/v1/health')) {
    return jsonResponse(health)
  }

  if (url.endsWith('/v1/admin/watchlist') && method === 'POST') {
    return jsonResponse({
      items: [
        ...initialWatchlist,
        { market: 'hk', name: '阿里巴巴-W', symbol: '09988.HK' },
      ],
    })
  }

  if (url.endsWith('/v1/admin/watchlist/600519.SH') && method === 'DELETE') {
    return jsonResponse({
      deleted: true,
      items: initialWatchlist.filter((item) => item.symbol !== '600519.SH'),
    })
  }

  if (url.endsWith('/v1/admin/watchlist')) {
    return jsonResponse({ items: initialWatchlist })
  }

  throw new Error(`unhandled request ${method} ${url}`)
}

function jsonResponse(body: unknown, init: ResponseInit = {}) {
  return Promise.resolve(
    new Response(JSON.stringify(body), {
      headers: { 'Content-Type': 'application/json' },
      status: 200,
      statusText: 'OK',
      ...init,
    }),
  )
}

function fetchCallsFor(path: string) {
  const fetchMock = vi.mocked(fetch)
  return fetchMock.mock.calls
    .filter(([input]) => String(input).endsWith(path))
    .map(([, init]) => ({
      headers: new Headers(init?.headers),
      method: init?.method ?? 'GET',
    }))
}
