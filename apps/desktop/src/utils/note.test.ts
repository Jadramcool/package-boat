import assert from 'node:assert/strict'
import test from 'node:test'
import { MAX_NOTE_BYTES, noteIsWithinLimit, utf8ByteLength } from './note.ts'

test('counts note limits as UTF-8 bytes', () => {
  assert.equal(utf8ByteLength('abc'), 3)
  assert.equal(utf8ByteLength('中文'), 6)
  assert.equal(utf8ByteLength('🚢'), 4)
  assert.equal(noteIsWithinLimit('中'.repeat(MAX_NOTE_BYTES / 3)), true)
  assert.equal(noteIsWithinLimit('中'.repeat(Math.floor(MAX_NOTE_BYTES / 3)) + 'aa'), false)
})

test('accepts exactly 64KB and rejects one extra byte', () => {
  assert.equal(noteIsWithinLimit('a'.repeat(MAX_NOTE_BYTES)), true)
  assert.equal(noteIsWithinLimit('a'.repeat(MAX_NOTE_BYTES + 1)), false)
})
