import type { FileListResponse, ServiceInfo, SessionStatus } from './types'

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

export function getServiceInfo(): Promise<ServiceInfo> {
  return request('/api/info')
}

export function getSession(): Promise<SessionStatus> {
  return request('/api/session')
}

export function pairDevice(code: string): Promise<void> {
  return request('/api/session', {
    method: 'POST',
    body: JSON.stringify({ code }),
  })
}

export function signOutDevice(): Promise<void> {
  return request('/api/session', { method: 'DELETE' })
}

export function getFiles(): Promise<FileListResponse> {
  return request('/api/files')
}

export function deleteSharedFile(id: string): Promise<void> {
  return request(`/api/files/${encodeURIComponent(id)}`, { method: 'DELETE' })
}
