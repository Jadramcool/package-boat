import {
  completeUploadSession,
  createUploadSession,
  getUploadSession,
  HttpError,
} from './api'
import type { ChunkUploadSession, UploadResponse } from './types'

interface ResumableUploadOptions {
  file: File
  sessionID?: string
  signal: AbortSignal
  onSession: (id: string) => void
  onProgress: (progress: number, resumed: boolean) => void
}

interface ResumableUploadResult {
  response: UploadResponse
  sessionID: string
}

export async function runResumableUpload(
  options: ResumableUploadOptions,
): Promise<ResumableUploadResult> {
  const { file, signal, onSession, onProgress } = options
  const session = await restoreOrCreateSession(file, signal, options.sessionID)
  onSession(session.id)

  const received = new Set(session.received_chunks)
  let confirmedBytes = session.uploaded_bytes
  const resumed = confirmedBytes > 0
  onProgress(progressPercent(confirmedBytes, file.size), resumed)

  for (let index = 0; index < session.total_chunks; index += 1) {
    throwIfAborted(signal)
    if (received.has(index))
      continue

    const start = index * session.chunk_size
    const chunk = file.slice(start, Math.min(start + session.chunk_size, file.size))
    const checksum = await sha256(chunk, signal)
    await uploadChunk(session, index, chunk, checksum, signal, (loaded) => {
      onProgress(progressPercent(confirmedBytes + loaded, file.size), resumed)
    })
    confirmedBytes += chunk.size
    received.add(index)
    onProgress(progressPercent(confirmedBytes, file.size), resumed)
  }

  throwIfAborted(signal)
  const response = await completeUploadSession(session.id, signal)
  onProgress(100, resumed)
  return { response, sessionID: session.id }
}

async function restoreOrCreateSession(
  file: File,
  signal: AbortSignal,
  sessionID?: string,
): Promise<ChunkUploadSession> {
  if (!sessionID)
    return createUploadSession(file, signal)

  try {
    const session = await getUploadSession(sessionID, signal)
    if (session.file_size === file.size)
      return session
  }
  catch (error) {
    if (!(error instanceof HttpError) || error.status !== 404)
      throw error
  }
  return createUploadSession(file, signal)
}

async function sha256(chunk: Blob, signal: AbortSignal): Promise<string> {
  throwIfAborted(signal)
  const bytes = await chunk.arrayBuffer()
  throwIfAborted(signal)
  const digest = globalThis.crypto?.subtle
    ? new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256', bytes))
    : (await import('@noble/hashes/sha2.js')).sha256(new Uint8Array(bytes))
  throwIfAborted(signal)
  return [...digest]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

function uploadChunk(
  session: ChunkUploadSession,
  index: number,
  chunk: Blob,
  checksum: string,
  signal: AbortSignal,
  onProgress: (loaded: number) => void,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest()
    const abort = () => request.abort()
    const cleanup = () => signal.removeEventListener('abort', abort)

    request.open(
      'PUT',
      `/api/uploads/${encodeURIComponent(session.id)}/chunks/${index}`,
    )
    request.withCredentials = true
    request.setRequestHeader('Content-Type', 'application/octet-stream')
    request.setRequestHeader('X-Chunk-SHA256', checksum)
    request.upload.addEventListener('progress', event => onProgress(event.loaded))
    request.addEventListener('load', () => {
      cleanup()
      if (request.status >= 200 && request.status < 300)
        resolve()
      else
        reject(xhrError(request))
    })
    request.addEventListener('error', () => {
      cleanup()
      reject(new Error('网络连接中断，请重试'))
    })
    request.addEventListener('abort', () => {
      cleanup()
      reject(new DOMException('上传已取消', 'AbortError'))
    })
    signal.addEventListener('abort', abort, { once: true })
    if (signal.aborted) {
      abort()
      return
    }
    request.send(chunk)
  })
}

function xhrError(request: XMLHttpRequest): HttpError {
  try {
    const payload = JSON.parse(request.responseText) as { error?: string }
    return new HttpError(request.status, payload.error ?? `上传失败（${request.status}）`)
  }
  catch {
    return new HttpError(request.status, `上传失败（${request.status || '网络错误'}）`)
  }
}

function progressPercent(done: number, total: number): number {
  if (total === 0)
    return 99
  return Math.min(99, Math.round(done / total * 100))
}

function throwIfAborted(signal: AbortSignal): void {
  if (signal.aborted)
    throw new DOMException('上传已取消', 'AbortError')
}
