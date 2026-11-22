import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { readFileSync } from 'fs'
import { resolve } from 'path'

const appVersion = readFileSync(resolve(__dirname, '../VERSION'), 'utf-8').trim()
const receiverPort = Number(process.env.ENGINEER_HTTP_PORT || '7437')
const devPort = Number(process.env.ENGINEER_DEV_PORT || '5173')
if (![receiverPort, devPort].every(port => Number.isInteger(port) && port > 0 && port <= 65535)) {
  throw new Error('Receiver and development ports must be between 1 and 65535')
}
const receiverOrigin = `http://127.0.0.1:${receiverPort}`

// https://vite.dev/config/
export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  plugins: [
    tailwindcss(),
    {
      name: 'engineer-local-api',
      configureServer(server) {
        server.middlewares.use((request, response, next) => {
          const origin = request.headers.origin
          if (request.url?.startsWith('/api/') && origin &&
              origin !== `http://127.0.0.1:${devPort}` && origin !== `http://localhost:${devPort}`) {
            response.statusCode = 403
            response.end('Forbidden')
            return
          }
          next()
        })
      },
    },
    react(),
  ],
  server: {
    host: '127.0.0.1',
    port: devPort,
    strictPort: true,
    cors: false,
    proxy: {
      '/api/': {
        target: receiverOrigin,
        changeOrigin: true,
        configure(proxy) {
          proxy.on('proxyReq', proxyRequest => proxyRequest.setHeader('Origin', receiverOrigin))
        },
      },
    },
  },
})
