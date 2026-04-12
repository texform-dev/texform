import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import wasmPackWatch from './vite-plugin-wasm-pack-watch'

const demoRoot = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(demoRoot, '..')
const wasmPkgRoot = resolve(repoRoot, 'crates/texform-wasm/pkg-web')

// https://vite.dev/config/
export default defineConfig({
  resolve: {
    alias: {
      'texform-wasm': resolve(wasmPkgRoot, 'texform_wasm.js'),
    },
  },
  server: {
    fs: {
      allow: [repoRoot],
    },
  },
  plugins: [react(), tailwindcss(), wasmPackWatch()],
})
