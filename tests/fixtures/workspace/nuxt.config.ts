import { fileURLToPath } from 'node:url'
import path from 'node:path'

const root = path.dirname(fileURLToPath(import.meta.url))

export default defineNuxtConfig({
  devtools: { enabled: true },
  alias: {
    '@assets': path.resolve(root, 'public'),
  },
  app: {
    head: {
      title: 'Sample Site',
    },
  },
})
