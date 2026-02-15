import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import wasmPackWatch from './vite-plugin-wasm-pack-watch'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss(), wasmPackWatch()],
})
