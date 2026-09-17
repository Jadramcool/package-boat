import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import {
  appendHistoryEntry,
  createHistoryEntry,
  loadHistory,
  saveHistory,
} from './transferHistory.ts'

function memoryStorage(initial: Record<string, string> = {}) {
  const map = new Map(Object.entries(initial))
  return {
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => {
      map.set(key, value)
    },
  }
}

describe('transferHistory', () => {
  it('prepends new entries and drops duplicates by id', () => {
    const first = createHistoryEntry({ id: 'a', name: 'a.bin', size: 1, status: 'complete' })
    const second = createHistoryEntry({ id: 'b', name: 'b.bin', size: 2, status: 'error', error: '网络中断' })
    const updatedFirst = createHistoryEntry({ id: 'a', name: 'a.bin', size: 1, status: 'cancelled' })

    let history = appendHistoryEntry([], first)
    history = appendHistoryEntry(history, second)
    assert.equal(history[0]?.id, 'b')
    history = appendHistoryEntry(history, updatedFirst)
    assert.equal(history.length, 2)
    assert.equal(history[0]?.id, 'a')
    assert.equal(history[0]?.status, 'cancelled')
    assert.equal(history[1]?.id, 'b')
  })

  it('caps history at 100 entries', () => {
    let history = appendHistoryEntry([], createHistoryEntry({ id: 'seed', name: 'seed', size: 0, status: 'complete' }))
    for (let index = 0; index < 120; index += 1) {
      history = appendHistoryEntry(history, createHistoryEntry({
        id: `id-${index}`,
        name: `file-${index}`,
        size: index,
        status: 'complete',
      }))
    }
    assert.equal(history.length, 100)
    assert.equal(history[0]?.id, 'id-119')
  })

  it('round-trips through storage and ignores corrupt payloads', () => {
    const storage = memoryStorage()
    const entry = createHistoryEntry({
      id: 'x',
      name: 'clip.mp4',
      size: 42,
      status: 'complete',
      finishedAt: new Date('2026-09-17T08:00:00.000Z'),
    })
    saveHistory([entry], storage)
    assert.deepEqual(loadHistory(storage), [entry])

    storage.setItem('packetboat.transferHistory.v1', '{not-json')
    assert.deepEqual(loadHistory(storage), [])

    storage.setItem('packetboat.transferHistory.v1', JSON.stringify([{ nope: true }, entry]))
    assert.deepEqual(loadHistory(storage), [entry])
  })
})
