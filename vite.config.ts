import { defineConfig } from 'vite'
import { resolve } from 'node:path'

export default defineConfig({ build: { rollupOptions: { input: { desktop: resolve(__dirname, 'index.html'), web: resolve(__dirname, 'web.html') } } } })
