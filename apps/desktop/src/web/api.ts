import type {
  ChunkUploadSession,
  CreateNotePayload,
  CreateNoteResponse,
  FileListResponse,
  NoteContentResponse,
  ServiceInfo,
  SessionStatus,
  TransferHistoryEntry,
  UploadResponse,
} from './types'

export class HttpError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message)
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    credentials: 'same-origin',
    ...init,
    headers: {
      ...(init?.body ? { 'Content-Type': 'application/json' } : {}),
      ...init?.headers,
    },
  })

  if (!response.ok) {
    const payload = await response.json().catch(() => ({ error: '请求失败，请稍后重试' })) as { error?: string }
    throw new HttpError(response.status, payload.error ?? '请求失败，请稍后重试')
  }

  if (response.status === 204)
    return undefined as T

  return response.json() as Promise<T>
}

/** 从 UA 提取设备名：Android 优先机型（如 PJZ110），否则「系统 + 浏览器」。 */
export function describeBrowserDevice(): string {
  const ua = navigator.userAgent
  const androidModel = extractAndroidModel(ua)
  if (androidModel)
    return androidModel
  const os = /iPad/i.test(ua)
    ? 'iPad'
    : /iPhone|iPod/i.test(ua)
      ? 'iPhone'
      : /Android/i.test(ua)
        ? 'Android'
        : /Mac OS|Macintosh/i.test(ua)
          ? 'macOS'
          : /Windows/i.test(ua)
            ? 'Windows'
            : /Linux/i.test(ua)
              ? 'Linux'
              : ''
  const browser = /Edg\//i.test(ua)
    ? 'Edge'
    : /Chrome/i.test(ua) && !/Chromium/i.test(ua)
      ? 'Chrome'
      : /Safari/i.test(ua) && !/Chrome/i.test(ua)
        ? 'Safari'
        : /Firefox/i.test(ua)
          ? 'Firefox'
          : ''
  return [os, browser].filter(Boolean).join(' ') || '浏览器设备'
}

function extractAndroidModel(ua: string): string | null {
  const start = ua.indexOf('Android ')
  if (start < 0)
    return null
  const rest = ua.slice(start + 'Android '.length)
  const segment = rest.split(';')[1]
  if (!segment)
    return null
  const model = segment.trim().split(' Build/')[0]?.trim().replace(/\)+$/, '') ?? ''
  if (!model || /^(wv|u)$/i.test(model) || model.length < 2)
    return null
  return model
}

export function getServiceInfo(): Promise<ServiceInfo> {
  return request('/api/info')
}

export function getSession(): Promise<SessionStatus> {
  return request('/api/session')
}

export function pairDevice(code: string): Promise<void> {
  return request('/api/session', {
    method: 'POST',
    body: JSON.stringify({
      code,
      device_label: describeBrowserDevice(),
    }),
  })
}

export function signOutDevice(): Promise<void> {
  return request('/api/session', { method: 'DELETE' })
}

export function getFiles(): Promise<FileListResponse> {
  return request('/api/files')
}

/** 主机侧传输历史（新到旧）。 */
export function getHistory(): Promise<{ history: TransferHistoryEntry[] }> {
  return request('/api/history')
}

/** 清空主机侧传输历史，返回删除条数。 */
export function clearHistory(): Promise<{ cleared: number }> {
  return request('/api/history', { method: 'DELETE' })
}

export function deleteSharedFile(id: string): Promise<void> {
  return request(`/api/files/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

/** 文本/剪贴板投递：写入接收目录并加入共享清单。 */
export function createNote(payload: CreateNotePayload): Promise<CreateNoteResponse> {
  return request('/api/notes', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/** 读取文本投递正文；不会触发下载历史或下载次数。 */
export function getNoteContent(id: string): Promise<NoteContentResponse> {
  return request(`/api/files/${encodeURIComponent(id)}/text`)
}

export function createUploadSession(file: File, signal?: AbortSignal): Promise<ChunkUploadSession> {
  return request('/api/uploads', {
    method: 'POST',
    signal,
    body: JSON.stringify({
      file_name: file.name,
      file_size: file.size,
    }),
  })
}

export function getUploadSession(id: string, signal?: AbortSignal): Promise<ChunkUploadSession> {
  return request(`/api/uploads/${encodeURIComponent(id)}`, { signal })
}

export function completeUploadSession(id: string, signal?: AbortSignal): Promise<UploadResponse> {
  return request(`/api/uploads/${encodeURIComponent(id)}/complete`, { method: 'POST', signal })
}

export function cancelUploadSession(id: string): Promise<void> {
  return request(`/api/uploads/${encodeURIComponent(id)}`, { method: 'DELETE' })
}
