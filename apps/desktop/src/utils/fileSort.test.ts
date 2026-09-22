import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { compareFiles, nextSortState, sortFiles } from './fileSort.ts'

const sample = [
  {
    name: 'b.zip',
    size: 20,
    source_type: 'received',
    is_dir: false,
    modified_at: '2026-01-02T00:00:00Z',
    download_count: 3,
  },
  {
    name: 'A.txt',
    size: 10,
    source_type: 'linked',
    is_dir: false,
    modified_at: '2026-01-03T00:00:00Z',
    download_count: 1,
  },
  {
    name: 'photos',
    size: 99,
    source_type: 'inbox',
    is_dir: true,
    modified_at: '2026-01-01T00:00:00Z',
    download_count: 5,
  },
]

describe('fileSort', () => {
  it('sorts by name ascending case-insensitively', () => {
    const sorted = sortFiles(sample, { key: 'name', direction: 'asc' })
    assert.deepEqual(sorted.map(item => item.name), ['A.txt', 'b.zip', 'photos'])
  })

  it('sorts by size descending', () => {
    const sorted = sortFiles(sample, { key: 'size', direction: 'desc' })
    assert.deepEqual(sorted.map(item => item.name), ['photos', 'b.zip', 'A.txt'])
  })

  it('sorts by source rank linked → received → inbox', () => {
    const sorted = sortFiles(sample, { key: 'source', direction: 'asc' })
    assert.deepEqual(sorted.map(item => item.source_type), ['linked', 'received', 'inbox'])
  })

  it('sorts by type folders first then extension', () => {
    const sorted = sortFiles(sample, { key: 'type', direction: 'asc' })
    assert.equal(sorted[0].name, 'photos')
    assert.equal(sorted[1].name, 'A.txt')
    assert.equal(sorted[2].name, 'b.zip')
  })

  it('sorts by download count', () => {
    const sorted = sortFiles(sample, { key: 'downloads', direction: 'desc' })
    assert.deepEqual(sorted.map(item => item.download_count), [5, 3, 1])
  })

  it('toggles direction when clicking the same key', () => {
    const next = nextSortState({ key: 'size', direction: 'desc' }, 'size')
    assert.deepEqual(next, { key: 'size', direction: 'asc' })
  })

  it('defaults name to ascending and size to descending', () => {
    assert.deepEqual(nextSortState({ key: 'size', direction: 'desc' }, 'name'), {
      key: 'name',
      direction: 'asc',
    })
    assert.deepEqual(nextSortState({ key: 'name', direction: 'asc' }, 'size'), {
      key: 'size',
      direction: 'desc',
    })
  })

  it('compareFiles is stable for equal ranks via name tie-break', () => {
    const a = { name: 'x.txt', size: 1, source_type: 'linked' }
    const b = { name: 'y.txt', size: 1, source_type: 'linked' }
    assert.ok(compareFiles(a, b, 'size', 'asc') < 0)
  })
})
