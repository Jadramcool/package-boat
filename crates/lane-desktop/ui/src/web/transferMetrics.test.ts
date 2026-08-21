import assert from 'node:assert/strict'
import test from 'node:test'
import { formatDuration } from '../utils/format.ts'
import { createTransferMetricsTracker } from './transferMetrics.ts'

test('calculates speed and ETA from a sliding sample window', () => {
  const tracker = createTransferMetricsTracker(10_000)

  assert.deepEqual(tracker.update(0, 0), {
    speedBytesPerSecond: 0,
    etaSeconds: undefined,
  })
  assert.deepEqual(tracker.update(1_000, 500), {
    speedBytesPerSecond: 2_000,
    etaSeconds: 5,
  })
  assert.deepEqual(tracker.update(3_000, 1_500), {
    speedBytesPerSecond: 2_000,
    etaSeconds: 4,
  })
})

test('drops stale speed after three seconds without progress', () => {
  const tracker = createTransferMetricsTracker(8_000)

  tracker.update(0, 0)
  assert.deepEqual(tracker.update(4_000, 1_000), {
    speedBytesPerSecond: 4_000,
    etaSeconds: 1,
  })
  assert.deepEqual(tracker.update(4_000, 4_000), {
    speedBytesPerSecond: 0,
    etaSeconds: undefined,
  })
})

test('resets its baseline when uploaded bytes move backwards on retry', () => {
  const tracker = createTransferMetricsTracker(10_000)

  tracker.update(5_000, 0)
  assert.deepEqual(tracker.update(1_000, 1_000), {
    speedBytesPerSecond: 0,
    etaSeconds: undefined,
  })
})

test('reports zero ETA after all bytes are uploaded', () => {
  const tracker = createTransferMetricsTracker(1_000)

  tracker.update(0, 0)
  assert.deepEqual(tracker.update(1_000, 500), {
    speedBytesPerSecond: 2_000,
    etaSeconds: 0,
  })
})

test('formats ETA for seconds, minutes and hours', () => {
  assert.equal(formatDuration(12.1), '13 秒')
  assert.equal(formatDuration(65), '1 分 5 秒')
  assert.equal(formatDuration(3_600), '1 小时')
  assert.equal(formatDuration(7_320), '2 小时 2 分')
})
