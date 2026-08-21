import { computed, onUnmounted, readonly, ref, shallowRef } from 'vue'
import {
  cancelUploadSession,
  deleteSharedFile,
  getFiles,
  getServiceInfo,
  getSession,
  HttpError,
  pairDevice,
  signOutDevice,
} from '../api'
import { runResumableUpload } from '../resumableUpload'
import type { ServiceInfo, SharedFile, UploadTask } from '../types'

const MAX_CONCURRENT_UPLOADS = 2

function taskID(): string {
  return globalThis.crypto?.randomUUID?.()
    ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : '发生未知错误'
}

function pairingCodeFromHash(): string {
  const hash = window.location.hash.startsWith('#')
    ? window.location.hash.slice(1)
    : window.location.hash
  const code = new URLSearchParams(hash).get('code')
  if (code === null)
    return ''

  window.history.replaceState(null, '', `${window.location.pathname}${window.location.search}`)
  const normalized = code.replace(/\D/g, '')
  return normalized.length === 6 ? normalized : ''
}

export function useFileShare() {
  const serviceInfo = shallowRef<ServiceInfo | null>(null)
  const files = shallowRef<SharedFile[]>([])
  const uploads = ref<UploadTask[]>([])
  const authenticated = shallowRef(false)
  const initializing = shallowRef(true)
  const pairing = shallowRef(false)
  const loadingFiles = shallowRef(false)
  const online = shallowRef(true)
  const error = shallowRef('')
  const deletingID = shallowRef('')
  const activeUploads = shallowRef(0)

  const uploadControllers = new Map<string, AbortController>()
  let events: EventSource | null = null

  const completedUploads = computed(() =>
    uploads.value.filter(task => task.status === 'complete').length,
  )
  const totalBytes = computed(() =>
    files.value.reduce((total, file) => total + file.size, 0),
  )

  async function initialize(): Promise<void> {
    initializing.value = true
    error.value = ''
    const code = pairingCodeFromHash()

    try {
      const [info, session] = await Promise.all([getServiceInfo(), getSession()])
      serviceInfo.value = info
      authenticated.value = session.authenticated
      online.value = true

      if (authenticated.value) {
        await loadFiles()
        connectEvents()
      }
      else if (code) {
        try {
          await pair(code)
        }
        catch {
          // Pairing exposes its own server-provided error in the gate.
        }
      }
    }
    catch (cause) {
      online.value = false
      error.value = errorMessage(cause)
    }
    finally {
      initializing.value = false
    }
  }

  async function pair(code: string): Promise<void> {
    pairing.value = true
    error.value = ''
    try {
      await pairDevice(code)
      authenticated.value = true
      online.value = true
      await loadFiles()
      connectEvents()
    }
    catch (cause) {
      error.value = errorMessage(cause)
      throw cause
    }
    finally {
      pairing.value = false
    }
  }

  async function signOut(): Promise<void> {
    try {
      await signOutDevice()
    }
    finally {
      disconnectEvents()
      for (const controller of uploadControllers.values())
        controller.abort()
      authenticated.value = false
      files.value = []
      error.value = ''
    }
  }

  async function loadFiles(): Promise<void> {
    loadingFiles.value = true
    try {
      const response = await getFiles()
      files.value = response.files
      online.value = true
    }
    catch (cause) {
      handleRequestError(cause)
    }
    finally {
      loadingFiles.value = false
    }
  }

  function addFiles(selected: readonly File[]): void {
    const maxSize = serviceInfo.value?.max_upload_bytes ?? Number.MAX_SAFE_INTEGER
    for (const file of selected) {
      const tooLarge = file.size > maxSize
      uploads.value.push({
        id: taskID(),
        file,
        progress: 0,
        status: tooLarge ? 'error' : 'queued',
        error: tooLarge ? '文件超过服务器大小限制' : undefined,
      })
    }
    scheduleUploads()
  }

  function scheduleUploads(): void {
    while (activeUploads.value < MAX_CONCURRENT_UPLOADS) {
      const task = uploads.value.find(candidate => candidate.status === 'queued')
      if (!task)
        return
      startUpload(task)
    }
  }

  function startUpload(task: UploadTask): void {
    void performUpload(task)
  }

  async function performUpload(task: UploadTask): Promise<void> {
    task.status = 'uploading'
    if (!task.sessionID)
      task.progress = 0
    task.error = undefined
    activeUploads.value += 1
    const controller = new AbortController()
    uploadControllers.set(task.id, controller)

    try {
      await runResumableUpload({
        file: task.file,
        sessionID: task.sessionID,
        signal: controller.signal,
        onSession: (sessionID) => {
          task.sessionID = sessionID
        },
        onProgress: (progress, resumed) => {
          task.progress = progress
          task.resumed = resumed
        },
      })
      task.status = 'complete'
      task.progress = 100
      task.sessionID = undefined
      online.value = true
      await loadFiles()
    }
    catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') {
        task.status = 'cancelled'
        task.error = undefined
        await discardUploadSession(task)
      }
      else {
        if (cause instanceof HttpError && cause.status === 401)
          authenticated.value = false
        task.status = 'error'
        task.error = errorMessage(cause)
        if (!(cause instanceof HttpError))
          online.value = false
      }
    }
    finally {
      uploadControllers.delete(task.id)
      activeUploads.value = Math.max(0, activeUploads.value - 1)
      scheduleUploads()
    }
  }

  function cancelUpload(id: string): void {
    const task = uploads.value.find(candidate => candidate.id === id)
    if (!task)
      return
    if (task.status === 'queued') {
      task.status = 'cancelled'
      void discardUploadSession(task)
      return
    }
    task.status = 'cancelled'
    uploadControllers.get(id)?.abort()
  }

  function retryUpload(id: string): void {
    const task = uploads.value.find(candidate => candidate.id === id)
    if (!task || !['error', 'cancelled'].includes(task.status))
      return
    task.status = 'queued'
    task.progress = 0
    task.error = undefined
    task.resumed = false
    scheduleUploads()
  }

  async function discardUploadSession(task: UploadTask): Promise<void> {
    const sessionID = task.sessionID
    task.sessionID = undefined
    if (!sessionID)
      return
    try {
      await cancelUploadSession(sessionID)
    }
    catch {
      // 会话会由服务端在 24 小时后清理；取消操作不覆盖用户界面状态。
    }
  }

  function clearFinishedUploads(): void {
    uploads.value = uploads.value.filter(task =>
      task.status !== 'complete' && task.status !== 'cancelled',
    )
  }

  async function deleteFile(id: string): Promise<void> {
    deletingID.value = id
    error.value = ''
    try {
      await deleteSharedFile(id)
      files.value = files.value.filter(file => file.id !== id)
    }
    catch (cause) {
      handleRequestError(cause)
    }
    finally {
      deletingID.value = ''
    }
  }

  function connectEvents(): void {
    disconnectEvents()
    events = new EventSource('/api/events')
    events.addEventListener('ready', () => {
      online.value = true
    })
    events.addEventListener('update', () => {
      online.value = true
      void loadFiles()
    })
    events.onerror = () => {
      online.value = false
    }
  }

  function disconnectEvents(): void {
    events?.close()
    events = null
  }

  function handleRequestError(cause: unknown): void {
    if (cause instanceof HttpError && cause.status === 401) {
      authenticated.value = false
      disconnectEvents()
    }
    error.value = errorMessage(cause)
  }

  onUnmounted(() => {
    disconnectEvents()
    for (const controller of uploadControllers.values())
      controller.abort()
  })

  return {
    serviceInfo: readonly(serviceInfo),
    files: readonly(files),
    uploads: readonly(uploads),
    authenticated: readonly(authenticated),
    initializing: readonly(initializing),
    pairing: readonly(pairing),
    loadingFiles: readonly(loadingFiles),
    online: readonly(online),
    error: readonly(error),
    deletingID: readonly(deletingID),
    completedUploads,
    totalBytes,
    initialize,
    pair,
    signOut,
    loadFiles,
    addFiles,
    cancelUpload,
    retryUpload,
    clearFinishedUploads,
    deleteFile,
  }
}
