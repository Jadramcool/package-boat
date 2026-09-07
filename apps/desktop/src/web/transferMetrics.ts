const WINDOW_MS = 4_000
const STALLED_AFTER_MS = 3_000
const MIN_SAMPLE_MS = 250

interface TransferSample {
  at: number
  bytes: number
}

export interface TransferMetrics {
  speedBytesPerSecond: number
  etaSeconds?: number
}

export interface TransferMetricsTracker {
  update: (uploadedBytes: number, now?: number) => TransferMetrics
}

/**
 * 使用最近约四秒的有效字节构建平滑速率。定时传入相同字节数时，旧样本会自然
 * 滑出窗口；连续三秒没有新数据后速度归零，避免弱网停顿时显示过期 ETA。
 */
export function createTransferMetricsTracker(totalBytes: number): TransferMetricsTracker {
  const total = normalizedBytes(totalBytes)
  let samples: TransferSample[] = []
  let lastIncreaseAt: number | undefined

  function update(uploadedBytes: number, now = performance.now()): TransferMetrics {
    const at = Number.isFinite(now) ? now : 0
    const bytes = Math.min(normalizedBytes(uploadedBytes), total)
    const previous = samples.at(-1)

    if (!previous || bytes < previous.bytes || at < previous.at) {
      samples = [{ at, bytes }]
      lastIncreaseAt = undefined
      return metricsFor(bytes, total, 0)
    }

    if (bytes > previous.bytes)
      lastIncreaseAt = at

    if (at === previous.at)
      previous.bytes = bytes
    else
      samples.push({ at, bytes })

    const windowStart = at - WINDOW_MS
    while (samples.length > 2 && samples[1].at <= windowStart)
      samples.shift()

    const first = samples[0]
    const latest = samples.at(-1)!
    const elapsed = latest.at - first.at
    let speed = elapsed >= MIN_SAMPLE_MS && latest.bytes > first.bytes
      ? (latest.bytes - first.bytes) / (elapsed / 1_000)
      : 0

    if (lastIncreaseAt === undefined || at - lastIncreaseAt >= STALLED_AFTER_MS)
      speed = 0

    return metricsFor(bytes, total, speed)
  }

  return { update }
}

function metricsFor(uploadedBytes: number, totalBytes: number, speed: number): TransferMetrics {
  const remainingBytes = Math.max(0, totalBytes - uploadedBytes)
  const speedBytesPerSecond = Number.isFinite(speed) && speed > 0 ? speed : 0
  return {
    speedBytesPerSecond,
    etaSeconds: remainingBytes === 0
      ? 0
      : speedBytesPerSecond > 0
        ? Math.ceil(remainingBytes / speedBytesPerSecond)
        : undefined,
  }
}

function normalizedBytes(value: number): number {
  return Number.isFinite(value) && value > 0 ? value : 0
}
