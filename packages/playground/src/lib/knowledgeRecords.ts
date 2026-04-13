import type { AllowedMode, BodyMode, CommandKind, ContextItem } from './texformWasm'
import { ParseContext } from './texformWasm'
import type { CustomKnowledgeRecordEntry } from './types'

const CUSTOM_KNOWLEDGE_RECORDS_STORAGE_KEY = 'texform-custom-knowledge-records'
const LEGACY_CUSTOM_COMMANDS_STORAGE_KEY = 'texform-custom-commands'

export function isAllowedMode(value: unknown): value is AllowedMode {
  return value === 'math' || value === 'text' || value === 'both'
}

export function isCommandKind(value: unknown): value is CommandKind {
  return value === 'prefix' || value === 'infix' || value === 'declarative'
}

export function isBodyMode(value: unknown): value is BodyMode {
  return value === 'math' || value === 'text'
}

export function recordIdentity(
  record: Pick<CustomKnowledgeRecordEntry, 'target' | 'name'>,
): string {
  return `${record.target}:${record.name}`
}

export function normalizeStoredCustomKnowledgeRecords(value: unknown): CustomKnowledgeRecordEntry[] {
  if (!Array.isArray(value)) {
    return []
  }

  const deduped = new Map<string, CustomKnowledgeRecordEntry>()

  for (const item of value) {
    if (typeof item !== 'object' || item === null) {
      continue
    }

    const candidate = item as Record<string, unknown>
    const name =
      typeof candidate.name === 'string'
        ? candidate.name.trim().replace(/^\\/, '')
        : ''
    if (!name) {
      continue
    }

    if (candidate.target === 'delimiter') {
      const record: CustomKnowledgeRecordEntry = {
        target: 'delimiter',
        name,
      }
      deduped.set(recordIdentity(record), record)
      continue
    }

    const spec = typeof candidate.argspec === 'string' ? candidate.argspec : ''
    const mode = candidate.mode
    if (!isAllowedMode(mode)) {
      continue
    }

    if (candidate.target === 'environment') {
      const bodyMode = candidate.bodyMode
      if (!isBodyMode(bodyMode)) {
        continue
      }

      const record: CustomKnowledgeRecordEntry = {
        target: 'environment',
        name,
        mode,
        bodyMode,
        argspec: spec,
      }
      deduped.set(recordIdentity(record), record)
      continue
    }

    const kind = candidate.kind
    if (!isCommandKind(kind)) {
      continue
    }

    const record: CustomKnowledgeRecordEntry = {
      target: 'command',
      name,
      kind,
      mode,
      argspec: spec,
    }
    deduped.set(recordIdentity(record), record)
  }

  return [...deduped.values()]
}

export function loadStoredCustomKnowledgeRecords(): CustomKnowledgeRecordEntry[] {
  try {
    const raw =
      localStorage.getItem(CUSTOM_KNOWLEDGE_RECORDS_STORAGE_KEY) ??
      localStorage.getItem(LEGACY_CUSTOM_COMMANDS_STORAGE_KEY)
    if (!raw) {
      return []
    }

    return normalizeStoredCustomKnowledgeRecords(JSON.parse(raw))
  } catch {
    return []
  }
}

export function persistCustomKnowledgeRecords(records: CustomKnowledgeRecordEntry[]): void {
  try {
    localStorage.setItem(CUSTOM_KNOWLEDGE_RECORDS_STORAGE_KEY, JSON.stringify(records))
    localStorage.removeItem(LEGACY_CUSTOM_COMMANDS_STORAGE_KEY)
  } catch {
    // Ignore storage quota errors
  }
}

export function customKnowledgeRecordToContextItem(
  record: CustomKnowledgeRecordEntry,
): ContextItem {
  if (record.target === 'command') {
    return {
      target: 'command',
      name: record.name,
      kind: record.kind,
      allowed_mode: record.mode,
      argspec: record.argspec,
    }
  }

  if (record.target === 'delimiter') {
    return {
      target: 'delimiter',
      name: record.name,
    }
  }

  return {
    target: 'environment',
    name: record.name,
    allowed_mode: record.mode,
    body_mode: record.bodyMode,
    argspec: record.argspec,
  }
}

export function buildParseContext(
  records: CustomKnowledgeRecordEntry[],
): ParseContext {
  const items = records.map(customKnowledgeRecordToContextItem)
  return new ParseContext(undefined, items)
}
