import { type Plugin } from 'vite'
import { resolve } from 'node:path'
import { exec } from 'node:child_process'

/**
 * Vite plugin that watches Rust source files and rebuilds the WASM package
 * automatically via wasm-pack, then triggers a full page reload.
 */
export default function wasmPackWatch(): Plugin {
  const projectRoot = resolve(__dirname, '../..')
  const outDir = resolve(projectRoot, 'crates/texform-wasm/pkg-web')

  const watchPaths = [
    'crates/texform-core/src',
    'crates/texform-interface/src',
    'crates/texform-specs/src',
    'crates/texform-wasm/src',
    'resources/specs',
  ].map((p) => resolve(projectRoot, p))

  let building = false

  function rebuild(server: { ws: { send: (msg: unknown) => void } }) {
    if (building) return
    building = true
    const cmd = `wasm-pack build crates/texform-wasm --target bundler --out-dir pkg-web`
    console.log('\x1b[36m[wasm-pack]\x1b[0m rebuilding...')
    const start = Date.now()
    exec(cmd, { cwd: projectRoot }, (err) => {
      building = false
      const elapsed = ((Date.now() - start) / 1000).toFixed(1)
      if (err) {
        console.log(`\x1b[33m[wasm-pack]\x1b[0m build failed (${elapsed}s) — waiting for next change`)
        return
      }
      console.log(`\x1b[32m[wasm-pack]\x1b[0m done in ${elapsed}s`)
      // Full reload — WASM module init state can't be HMR'd cleanly
      server.ws.send({ type: 'full-reload', path: '*' })
    })
  }

  return {
    name: 'vite-plugin-wasm-pack-watch',
    apply: 'serve',

    configureServer(server) {
      // Initial build on dev server start
      rebuild(server)

      for (const dir of watchPaths) {
        server.watcher.add(dir)
      }

      server.watcher.on('change', (file) => {
        // Only react to Rust / YAML source changes, ignore pkg output
        if (file.startsWith(outDir)) return
        const isRust = file.endsWith('.rs')
        const isYaml = file.endsWith('.yaml') || file.endsWith('.yml')
        if (isRust || isYaml) {
          rebuild(server)
        }
      })
    },
  }
}
