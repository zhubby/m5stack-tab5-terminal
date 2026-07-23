import react from '@vitejs/plugin-react'
import { loadEnv } from 'vite'
import { defineConfig } from 'vitest/config'

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')
  const backendOrigin = env.VITE_BACKEND_ORIGIN ?? 'http://127.0.0.1:8080'

  return {
    plugins: [react()],
    server: {
      proxy: {
        '/v1': {
          target: backendOrigin,
          changeOrigin: true,
          ws: true,
        },
      },
    },
    test: {
      css: true,
      environment: 'jsdom',
      globals: true,
      setupFiles: './src/test/setup.ts',
    },
  }
})
