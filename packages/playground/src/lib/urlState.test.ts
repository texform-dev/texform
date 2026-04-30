import { afterEach, describe, expect, test } from 'bun:test'
import { readStateFromUrl } from './urlState'

const originalWindow = globalThis.window

function setSearch(search: string): void {
  Object.defineProperty(globalThis, 'window', {
    value: { location: { search } },
    configurable: true,
  })
}

afterEach(() => {
  Object.defineProperty(globalThis, 'window', {
    value: originalWindow,
    configurable: true,
  })
})

describe('readStateFromUrl', () => {
  test('ignores serialize opts that do not match the schema', () => {
    setSearch(
      `?opts=${encodeURIComponent(
        JSON.stringify({
          math: {
            spacing: {
              commands: 'wide',
              group_inner_spacing: 'padded',
              adjacent_chars: 'spaced',
            },
            scripts: { spacing: 'spaced', order: 'sub_first' },
          },
          syntax: { environments: { name_spacing: 'spaced' } },
        }),
      )}`,
    )

    expect(readStateFromUrl().serializeOptions).toBeUndefined()
  })
})
