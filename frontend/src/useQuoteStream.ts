import { useEffect, useState } from 'react'
import type { Quote, StreamMessage } from './types'

export interface QuoteStreamState {
  error: string | null
  label: string
  tone: 'neutral' | 'good' | 'bad'
}

interface UseQuoteStreamOptions {
  onQuote: (quote: Quote) => void
  onSnapshot: (quotes: Quote[]) => void
  token: string | null
}

export function useQuoteStream({ onQuote, onSnapshot, token }: UseQuoteStreamOptions) {
  const [state, setState] = useState<QuoteStreamState>({
    error: null,
    label: '连接中',
    tone: 'neutral',
  })

  useEffect(() => {
    if (!token?.trim()) {
      setState({ error: null, label: '等待 token', tone: 'neutral' })
      return
    }

    let socket: WebSocket | null = null
    let reconnectTimer: number | undefined
    let closedByEffect = false

    const connect = () => {
      socket = new WebSocket(buildStreamUrl(token))
      setState({ error: null, label: '连接中', tone: 'neutral' })

      socket.addEventListener('open', () => {
        setState({ error: null, label: 'connected', tone: 'good' })
      })

      socket.addEventListener('message', (event) => {
        try {
          const message = JSON.parse(String(event.data)) as StreamMessage
          if (message.type === 'snapshot') {
            onSnapshot(message.quotes)
          } else if (message.type === 'quote') {
            onQuote(message.quote)
          } else if (message.type === 'status') {
            setState({ error: null, label: message.status, tone: 'good' })
          } else if (message.type === 'error') {
            setState({ error: message.message, label: 'error', tone: 'bad' })
          }
        } catch (error) {
          setState({
            error: error instanceof Error ? error.message : '行情消息解析失败',
            label: 'error',
            tone: 'bad',
          })
        }
      })

      socket.addEventListener('error', () => {
        setState({ error: '行情连接错误', label: 'error', tone: 'bad' })
      })

      socket.addEventListener('close', () => {
        if (closedByEffect) {
          return
        }
        setState((current) => ({
          error: current.error,
          label: 'closed',
          tone: 'neutral',
        }))
        reconnectTimer = window.setTimeout(connect, 2500)
      })
    }

    connect()

    return () => {
      closedByEffect = true
      if (reconnectTimer !== undefined) {
        window.clearTimeout(reconnectTimer)
      }
      socket?.close()
    }
  }, [onQuote, onSnapshot, token])

  return state
}

function buildStreamUrl(token: string) {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const params = token.trim() ? `?token=${encodeURIComponent(token.trim())}` : ''
  return `${protocol}//${window.location.host}/v1/quotes/stream${params}`
}
