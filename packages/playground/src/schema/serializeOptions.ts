import { z } from 'zod'

// --- Individual field schemas with Title|Description metadata ---

export const commandSpacingSchema = z
  .enum(['spaced', 'minimal'])
  .describe('Command Spacing|Space between command name and its argument braces')

export const mathGroupInnerSpacingSchema = z
  .enum(['padded', 'compact'])
  .describe('Group Inner Spacing|Padding inside math group braces')

export const adjacentCharSpacingSchema = z
  .enum(['spaced', 'compact'])
  .describe('Adjacent Characters|Spacing between consecutive math characters')

export const scriptSpacingSchema = z
  .enum(['spaced', 'compact'])
  .describe('Script Spacing|Space around subscript/superscript operators')

export const scriptOrderSchema = z
  .enum(['sub_first', 'sup_first'])
  .describe('Script Order|Output order of subscript and superscript')

export const environmentNameSpacingSchema = z
  .enum(['spaced', 'compact'])
  .describe(
    'Environment Name Spacing|Space between \\begin/\\end and environment name',
  )

// --- Composite schema ---

export const serializeOptionsSchema = z.object({
  math: z.object({
    spacing: z.object({
      commands: commandSpacingSchema.default('spaced'),
      group_inner_spacing: mathGroupInnerSpacingSchema.default('padded'),
      adjacent_chars: adjacentCharSpacingSchema.default('spaced'),
    }),
    scripts: z.object({
      spacing: scriptSpacingSchema.default('spaced'),
      order: scriptOrderSchema.default('sub_first'),
    }),
  }),
  syntax: z.object({
    environments: z.object({
      name_spacing: environmentNameSpacingSchema.default('spaced'),
    }),
  }),
})

export type SerializeOptions = z.infer<typeof serializeOptionsSchema>

// --- Flat option descriptor for UI generation ---

export interface OptionDescriptor {
  path: string
  title: string
  description: string
  values: readonly string[]
  defaultValue: string
  disabled?: boolean
}

export function parseFieldMeta(desc: string): {
  title: string
  description: string
} {
  const [title, ...rest] = desc.split('|')
  return { title: title.trim(), description: rest.join('|').trim() }
}

/** Flat list of all serialize options for UI rendering. */
export const SERIALIZE_OPTION_DESCRIPTORS: OptionDescriptor[] = [
  {
    ...parseFieldMeta(commandSpacingSchema.description!),
    path: 'math.spacing.commands',
    values: commandSpacingSchema.options,
    defaultValue: 'spaced',
  },
  {
    ...parseFieldMeta(mathGroupInnerSpacingSchema.description!),
    path: 'math.spacing.group_inner_spacing',
    values: mathGroupInnerSpacingSchema.options,
    defaultValue: 'padded',
  },
  {
    ...parseFieldMeta(adjacentCharSpacingSchema.description!),
    path: 'math.spacing.adjacent_chars',
    values: adjacentCharSpacingSchema.options,
    defaultValue: 'spaced',
  },
  {
    ...parseFieldMeta(scriptSpacingSchema.description!),
    path: 'math.scripts.spacing',
    values: scriptSpacingSchema.options,
    defaultValue: 'spaced',
  },
  {
    ...parseFieldMeta(scriptOrderSchema.description!),
    path: 'math.scripts.order',
    values: scriptOrderSchema.options,
    defaultValue: 'sub_first',
  },
  {
    ...parseFieldMeta(environmentNameSpacingSchema.description!),
    path: 'syntax.environments.name_spacing',
    values: environmentNameSpacingSchema.options,
    defaultValue: 'spaced',
  },
]

/** Get a nested value from SerializeOptions by dot-path. */
export function getOptionValue(
  options: SerializeOptions,
  path: string,
): string {
  const parts = path.split('.')
  let current: unknown = options
  for (const part of parts) {
    if (current == null || typeof current !== 'object') return ''
    current = (current as Record<string, unknown>)[part]
  }
  return typeof current === 'string' ? current : ''
}

/** Return a new SerializeOptions with a single field updated by dot-path. */
export function setOptionValue(
  options: SerializeOptions,
  path: string,
  value: string,
): SerializeOptions {
  const result = structuredClone(options)
  const parts = path.split('.')
  let current: Record<string, unknown> = result as Record<string, unknown>
  for (let i = 0; i < parts.length - 1; i++) {
    current = current[parts[i]] as Record<string, unknown>
  }
  current[parts[parts.length - 1]] = value
  return result
}

/** Default serialize options (all fields at their default values). */
export const DEFAULT_SERIALIZE_OPTIONS: SerializeOptions =
  serializeOptionsSchema.parse({
    math: { spacing: {}, scripts: {} },
    syntax: { environments: {} },
  })
