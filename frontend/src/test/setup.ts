import '@testing-library/jest-dom/vitest'

const values = new Map<string, string>()

const localStorageMock: Storage = {
  get length() {
    return values.size
  },
  clear() {
    values.clear()
  },
  getItem(key: string) {
    return values.get(key) ?? null
  },
  key(index: number) {
    return Array.from(values.keys())[index] ?? null
  },
  removeItem(key: string) {
    values.delete(key)
  },
  setItem(key: string, value: string) {
    values.set(key, value)
  },
}

Object.defineProperty(globalThis, 'localStorage', {
  configurable: true,
  value: localStorageMock,
})

Object.defineProperty(window, 'localStorage', {
  configurable: true,
  value: localStorageMock,
})
