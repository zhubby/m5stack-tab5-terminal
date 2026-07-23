import {
  Activity,
  AlertTriangle,
  Database,
  KeyRound,
  Plus,
  RefreshCw,
  Trash2,
  Wifi,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import './App.css'
import {
  deleteWatchItem,
  fetchAdminWatchlist,
  fetchHealth,
  getStoredToken,
  setStoredToken,
  upsertWatchItem,
} from './api'
import type { HealthResponse, Quote, QuoteStatus, WatchlistItem } from './types'
import { useQuoteStream } from './useQuoteStream'

type LoadState = 'idle' | 'loading' | 'ready' | 'error'

const statusLabels: Record<QuoteStatus, string> = {
  normal: '正常',
  stale: '过期',
  offline: '离线',
  market_closed: '休市',
  suspended: '停牌',
}

function App() {
  const [token, setToken] = useState(() => getStoredToken())
  const [streamToken, setStreamToken] = useState<string | null>(null)
  const [health, setHealth] = useState<HealthResponse | null>(null)
  const [watchlist, setWatchlist] = useState<WatchlistItem[]>([])
  const [quotes, setQuotes] = useState<Record<string, Quote>>({})
  const [symbol, setSymbol] = useState('')
  const [name, setName] = useState('')
  const [notice, setNotice] = useState('等待同步')
  const [loadState, setLoadState] = useState<LoadState>('idle')
  const [isSaving, setIsSaving] = useState(false)

  const handleSnapshot = useCallback((nextQuotes: Quote[]) => {
    setQuotes(Object.fromEntries(nextQuotes.map((quote) => [quote.symbol, quote])))
  }, [])

  const handleQuote = useCallback((quote: Quote) => {
    setQuotes((current) => ({ ...current, [quote.symbol]: quote }))
  }, [])

  const stream = useQuoteStream({
    token: streamToken,
    onQuote: handleQuote,
    onSnapshot: handleSnapshot,
  })

  const loadDashboard = useCallback(async (nextToken: string, quiet = false) => {
    setLoadState('loading')
    if (!quiet) {
      setNotice('同步中')
    }

    try {
      const [nextHealth, nextWatchlist] = await Promise.all([
        fetchHealth(nextToken),
        fetchAdminWatchlist(nextToken),
      ])
      setHealth(nextHealth)
      setWatchlist(nextWatchlist.items)
      setLoadState('ready')
      setNotice('已同步')
      setStreamToken(nextToken)
    } catch (error) {
      setLoadState('error')
      setStreamToken(null)
      setNotice(toErrorMessage(error))
    }
  }, [])

  useEffect(() => {
    const storedToken = getStoredToken()
    void loadDashboard(storedToken, true)
  }, [loadDashboard])

  const rows = useMemo(
    () =>
      watchlist.map((item) => ({
        item,
        quote: quotes[item.symbol],
      })),
    [quotes, watchlist],
  )

  const handleTokenChange = (value: string) => {
    setToken(value)
    setStoredToken(value)
  }

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setIsSaving(true)
    setNotice('保存中')

    try {
      const response = await upsertWatchItem(token, {
        symbol,
        name: name.trim() ? name : null,
      })
      setWatchlist(response.items)
      setSymbol('')
      setName('')
      setNotice('已保存')
      setStreamToken(token)
    } catch (error) {
      setNotice(toErrorMessage(error))
    } finally {
      setIsSaving(false)
    }
  }

  const handleDelete = async (deleteSymbol: string) => {
    setNotice('删除中')

    try {
      const response = await deleteWatchItem(token, deleteSymbol)
      setWatchlist(response.items)
      setQuotes((current) => {
        const next = { ...current }
        delete next[deleteSymbol]
        return next
      })
      setNotice(response.deleted ? '已删除' : '未找到')
      setStreamToken(token)
    } catch (error) {
      setNotice(toErrorMessage(error))
    }
  }

  const handleRefresh = async () => {
    await loadDashboard(token)
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <h1>Tab5 股票终端</h1>
          <p className="subline">自选股管理</p>
        </div>
        <div className="status-strip" aria-live="polite">
          <StatusPill
            icon={<Activity size={16} />}
            label="后端"
            tone={loadState === 'error' ? 'bad' : 'neutral'}
            value={health?.status ?? (loadState === 'loading' ? '同步中' : '--')}
          />
          <StatusPill icon={<Wifi size={16} />} label="行情" tone={stream.tone} value={stream.label} />
          <StatusPill
            icon={<Database size={16} />}
            label="自选"
            tone="neutral"
            value={String(watchlist.length)}
          />
        </div>
      </header>

      <section className="workspace" aria-label="自选股管理台">
        <aside className="control-panel" aria-labelledby="edit-title">
          <div className="panel-head">
            <h2 id="edit-title">编辑</h2>
          </div>
          <form className="edit-form" onSubmit={handleSubmit}>
            <label>
              <span>代码</span>
              <input
                autoComplete="off"
                onChange={(event) => setSymbol(event.target.value)}
                placeholder="600519.SH / 00700.HK"
                required
                value={symbol}
              />
            </label>
            <label>
              <span>名称</span>
              <input
                autoComplete="off"
                onChange={(event) => setName(event.target.value)}
                placeholder="贵州茅台"
                value={name}
              />
            </label>
            <label>
              <span>Token</span>
              <div className="token-field">
                <KeyRound size={16} aria-hidden="true" />
                <input
                  autoComplete="off"
                  onChange={(event) => handleTokenChange(event.target.value)}
                  type="password"
                  value={token}
                />
              </div>
            </label>
            <div className="actions">
              <button className="primary" disabled={isSaving} type="submit">
                <Plus size={16} aria-hidden="true" />
                {isSaving ? '保存中' : '添加'}
              </button>
              <button disabled={loadState === 'loading'} onClick={handleRefresh} type="button">
                <RefreshCw size={16} aria-hidden="true" />
                刷新
              </button>
            </div>
            <p
              className={loadState === 'error' || stream.tone === 'bad' ? 'notice is-error' : 'notice'}
              role="status"
            >
              {loadState === 'error' ? notice : (stream.error ?? notice)}
            </p>
          </form>
        </aside>

        <section className="table-panel" aria-labelledby="list-title">
          <div className="panel-head">
            <h2 id="list-title">自选列表</h2>
            <span className="muted">{formatTime(health?.server_ts)}</span>
          </div>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>代码</th>
                  <th>名称</th>
                  <th>市场</th>
                  <th className="num">最新价</th>
                  <th className="num">涨跌额</th>
                  <th className="num">涨跌幅</th>
                  <th className="num">成交额</th>
                  <th>更新</th>
                  <th>状态</th>
                  <th className="action-col">操作</th>
                </tr>
              </thead>
              <tbody>
                {rows.map(({ item, quote }) => (
                  <QuoteRow
                    item={item}
                    key={item.symbol}
                    onDelete={handleDelete}
                    quote={quote}
                  />
                ))}
              </tbody>
            </table>
            {rows.length === 0 ? (
              <div className="empty">
                <AlertTriangle size={18} aria-hidden="true" />
                暂无自选股
              </div>
            ) : null}
          </div>
        </section>
      </section>
    </main>
  )
}

function StatusPill({
  icon,
  label,
  tone,
  value,
}: {
  icon: React.ReactNode
  label: string
  tone: 'neutral' | 'good' | 'bad'
  value: string
}) {
  return (
    <span className={`pill ${tone}`} data-testid={`${label}-status`}>
      {icon}
      <span>{label}</span>
      <strong>{value}</strong>
    </span>
  )
}

function QuoteRow({
  item,
  onDelete,
  quote,
}: {
  item: WatchlistItem
  onDelete: (symbol: string) => void
  quote?: Quote
}) {
  const direction = quote ? quoteDirection(quote) : ''
  const status = quote ? quote.status : undefined

  return (
    <tr>
      <td className="symbol">{item.symbol}</td>
      <td>{item.name}</td>
      <td>{item.market.toUpperCase()}</td>
      <td className={`num ${direction}`}>{formatNumber(quote?.last)}</td>
      <td className={`num ${direction}`}>{formatSignedNumber(quote?.change)}</td>
      <td className={`num ${direction}`}>{formatPercent(quote?.change_pct)}</td>
      <td className="num">{formatTurnover(quote?.turnover)}</td>
      <td className="muted">{formatTime(quote?.server_ts)}</td>
      <td>
        <span className={`quote-status ${status ?? 'empty'}`}>
          {status ? statusLabels[status] : '--'}
        </span>
      </td>
      <td className="action-col">
        <button
          aria-label={`删除 ${item.symbol}`}
          className="danger icon-button"
          onClick={() => onDelete(item.symbol)}
          title="删除"
          type="button"
        >
          <Trash2 size={16} aria-hidden="true" />
        </button>
      </td>
    </tr>
  )
}

function quoteDirection(quote: Quote) {
  if (quote.change > 0) {
    return 'up'
  }
  if (quote.change < 0) {
    return 'down'
  }
  return ''
}

function formatNumber(value: number | undefined) {
  return typeof value === 'number' ? value.toFixed(2) : '--'
}

function formatSignedNumber(value: number | undefined) {
  if (typeof value !== 'number') {
    return '--'
  }
  return `${value > 0 ? '+' : ''}${value.toFixed(2)}`
}

function formatPercent(value: number | undefined) {
  if (typeof value !== 'number') {
    return '--'
  }
  return `${value > 0 ? '+' : ''}${value.toFixed(2)}%`
}

function formatTurnover(value: number | undefined) {
  if (typeof value !== 'number') {
    return '--'
  }
  if (value >= 100_000_000) {
    return `${(value / 100_000_000).toFixed(2)}亿`
  }
  if (value >= 10_000) {
    return `${(value / 10_000).toFixed(1)}万`
  }
  return value.toFixed(0)
}

function formatTime(value: string | null | undefined) {
  if (!value) {
    return '--'
  }
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return '--'
  }
  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    hour12: false,
    minute: '2-digit',
    second: '2-digit',
  }).format(date)
}

function toErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : '请求失败'
}

export default App
