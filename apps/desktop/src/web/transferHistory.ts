import type { TransferHistoryEntry, TransferHistoryStatus } from './types'

const STORAGE_KEY = 'packetboat.transferHistory.v1'
const MAX_ENTRIES = 100

export function createHistoryEntry(input: {
  id: string
  name: string
  size: number
  status: TransferHistoryStatus
  error?: string
  finishedAt?: Date
}): TransferHistoryEntry {
  const entry: TransferHistoryEntry = {
    id: input.id,
    name: input.name,
    size: input.size,
    status: input.status,
    finishedAt: (input.finishedAt ?? new Date()).toISOString(),
  }
  if (input.error !== undefined)
    entry.error = input.error
  return entry
}

export function appendHistoryEntry(
  history: readonly TransferHistoryEntry[],
  entry: TransferHistoryEntry,
): TransferHistoryEntry[] {
  const next = history.filter(item => item.id !== entry.id)
  next.unshift(entry)
  return next.slice(0, MAX_ENTRIES)
}

export function loadHistory(storage: Pick<Storage, 'getItem'> = localStorage): TransferHistoryEntry[] {
  try {
    const raw = storage.getItem(STORAGE_KEY)
    if (!raw)
      return []
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed))
      return []
    return parsed.filter(isHistoryEntry).slice(0, MAX_ENTRIES)
  }
  catch {
    return []
  }
}

export function saveHistory(
  history: readonly TransferHistoryEntry[],
  storage: Pick<Storage, 'setItem'> = localStorage,
): void {
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(history))
  }
  catch {
    // 隐私模式或配额不足时静默降级为仅内存历史。
  }
}

function isHistoryEntry(value: unknown): value is TransferHistoryEntry {
  if (typeof value !== 'object' || value === null)
    return false
  const entry = value as Partial<TransferHistoryEntry>
  return typeof entry.id === 'string'
    && typeof entry.name === 'string'
    && typeof entry.size === 'number'
    && (entry.status === 'complete' || entry.status === 'error' || entry.status === 'cancelled')
    && typeof entry.finishedAt === 'string'
}
