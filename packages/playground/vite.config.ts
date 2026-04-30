import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import wasm from 'vite-plugin-wasm'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import wasmPackWatch from './vite-plugin-wasm-pack-watch'

const pkgRoot = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolve(pkgRoot, '../..')
const wasmPkgRoot = resolve(repoRoot, 'crates/texform-wasm/pkg-web')

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
  plugins: [wasm(), react(), tailwindcss(), wasmPackWatch()],
})
