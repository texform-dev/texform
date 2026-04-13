import type { SerializeOptions } from '../schema/serializeOptions'
import { DEFAULT_SERIALIZE_OPTIONS } from '../schema/serializeOptions'

interface PlaygroundState {
  source: string
  strict: boolean
  tab: 'tree' | 'serialized'
  serializeOptions: SerializeOptions
}

const DEFAULTS: PlaygroundState = {
  source: '',
  strict: false,
  tab: 'tree',
  serializeOptions: DEFAULT_SERIALIZE_OPTIONS,
}

/** Read playground state from URL search params. */
export function readStateFromUrl(): Partial<PlaygroundState> {
  const params = new URLSearchParams(window.location.search)
  const state: Partial<PlaygroundState> = {}

  const src = params.get('src')
  if (src !== null) state.source = src

  if (params.get('strict') === '1') state.strict = true

  const tab = params.get('tab')
  if (tab === 'serialized') state.tab = 'serialized'

  const opts = params.get('opts')
  if (opts) {
    try {
      state.serializeOptions = JSON.parse(opts) as SerializeOptions
    } catch {
      // Ignore malformed opts
    }
  }

  return state
}

/** Encode playground state to a shareable URL. */
export function buildShareUrl(state: PlaygroundState): string {
  const url = new URL(window.location.href)
  url.search = ''

  if (state.source) url.searchParams.set('src', state.source)
  if (state.strict) url.searchParams.set('strict', '1')
  if (state.tab !== 'tree') url.searchParams.set('tab', state.tab)

  const optsJson = JSON.stringify(state.serializeOptions)
  const defaultJson = JSON.stringify(DEFAULTS.serializeOptions)
  if (optsJson !== defaultJson) {
    url.searchParams.set('opts', optsJson)
  }

  return url.toString()
}

/** Copy text to clipboard. Returns true on success. */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}
