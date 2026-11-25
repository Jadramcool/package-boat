import { computed, onUnmounted, readonly, ref, shallowRef, watch } from 'vue'
import {
  cancelUploadSession,
  clearHistory as clearHistoryRequest,
  createNote,
  deleteSharedFile,
  getFiles,
  getHistory,
  getServiceInfo,
  getSession,
  HttpError,
  pairDevice,
  signOutDevice,
} from '../api'
import { runResumableUpload } from '../resumableUpload'
import { createTransferMetricsTracker } from '../transferMetrics'
import type { ServiceInfo, SharedFile, TransferHistoryEntry, UploadTask } from '../types'

const DEFAULT_CONCURRENT_UPLOADS = 2
const MIN_CONCURRENT_UPLOADS = 1
const MAX_CONCURRENT_UPLOADS = 8
const CONCURRENCY_STORAGE_KEY = 'packetboat.uploadConcurrency'

function taskID(): string {
  return globalThis.crypto?.randomUUID?.()
    ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : '发生未知错误'
}

/** 把浏览器网络错误映射为可读中文提示。 */
function friendlyNetworkMessage(): string {
  return '无法连接主机服务。请确认：1）电脑上 PacketBoat 仍在运行；2）手机与电脑在同一局域网；3）访问地址与端口未变。恢复后页面会自动重连。'
}

/** 大文件下载占满浏览器连接、或主机暂时不可达时，fetch 抛出的网络类错误。 */
function isTransientNetworkError(cause: unknown): boolean {
  if (cause instanceof TypeError)
    return true
  if (cause instanceof Error) {
    const message = cause.message.toLowerCase()
    return message.includes('failed to fetch')
      || message.includes('networkerror')
      || message.includes('load failed')
      || message.includes('network request failed')
  }
  return false
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

function normalizeConcurrency(value: unknown): number {
  const number = typeof value === 'number' ? value : Number(value)
  if (!Number.isFinite(number))
    return DEFAULT_CONCURRENT_UPLOADS
  return Math.min(MAX_CONCURRENT_UPLOADS, Math.max(MIN_CONCURRENT_UPLOADS, Math.round(number)))
}

function loadConcurrency(): number {
  try {
    const raw = localStorage.getItem(CONCURRENCY_STORAGE_KEY)
    return raw === null ? DEFAULT_CONCURRENT_UPLOADS : normalizeConcurrency(raw)
  }
  catch {
    return DEFAULT_CONCURRENT_UPLOADS
  }
}

function persistConcurrency(value: number): void {
  try {
    localStorage.setItem(CONCURRENCY_STORAGE_KEY, String(value))
  }
  catch {
    // 隐私模式下忽略持久化失败。
  }
}

export function useFileShare() {
  const serviceInfo = shallowRef<ServiceInfo | null>(null)
  const files = shallowRef<SharedFile[]>([])
  const uploads = ref<UploadTask[]>([])
  const history = shallowRef<TransferHistoryEntry[]>([])
  const authenticated = shallowRef(false)
  const initializing = shallowRef(true)
  const pairing = shallowRef(false)
  const loadingFiles = shallowRef(false)
  const online = shallowRef(true)
  const error = shallowRef('')
  const deletingID = shallowRef('')
  const activeUploads = shallowRef(0)
  const uploadConcurrency = shallowRef(loadConcurrency())

  const uploadControllers = new Map<string, AbortController>()
  const abortIntents = new Map<string, 'pause' | 'cancel'>()
  let events: EventSource | null = null
  let heartbeatTimer = 0
  let loadFilesTimer = 0
  let reconnectTimer = 0
  let offlineGraceTimer = 0
  /** 连续网络失败次数；达到阈值才标离线（点下载瞬间的瞬时失败会忽略）。 */
  let networkFailCount = 0

  const completedUploads = computed(() =>
    uploads.value.filter(task => task.status === 'complete').length,
  )
  const totalBytes = computed(() =>
    files.value.reduce((total, file) => total + file.size, 0),
  )

  /** 方案 C：下载与页面同源同端口；连接池由服务端每 IP 限并发保护。 */
  const downloadOrigin = computed(() => window.location.origin)

  const isNetworkBanner = computed(() => error.value === friendlyNetworkMessage())

  watch(uploadConcurrency, (value) => {
    persistConcurrency(value)
    scheduleUploads()
  })

  watch(online, (value) => {
    if (value) {
      stopReconnectRetry()
      if (isNetworkBanner.value)
        error.value = ''
    }
    else {
      startReconnectRetry()
    }
  })

  /** 一次网络失败：首次 blip 不切换「重连中」。 */
  function noteNetworkFailure(showBanner: boolean): void {
    networkFailCount += 1
    if (networkFailCount < 2)
      return
    online.value = false
    if (showBanner)
      error.value = friendlyNetworkMessage()
    startReconnectRetry()
  }

  function noteNetworkSuccess(): void {
    networkFailCount = 0
    online.value = true
  }

  function startReconnectRetry(): void {
    if (reconnectTimer)
      return
    reconnectTimer = window.setInterval(() => {
      void tryReconnect()
    }, 4000)
  }

  function stopReconnectRetry(): void {
    if (reconnectTimer) {
      window.clearInterval(reconnectTimer)
      reconnectTimer = 0
    }
  }

  async function tryReconnect(): Promise<void> {
    try {
      const [info, session] = await Promise.all([getServiceInfo(), getSession()])
      serviceInfo.value = info
      authenticated.value = session.authenticated
      noteNetworkSuccess()
      if (authenticated.value) {
        await loadFiles()
        await loadHistory()
        connectEvents()
      }
    }
    catch {
      noteNetworkFailure(true)
    }
  }

  async function initialize(): Promise<void> {
    initializing.value = true
    error.value = ''
    networkFailCount = 0
    const code = pairingCodeFromHash()

    try {
      const [info, session] = await Promise.all([getServiceInfo(), getSession()])
      serviceInfo.value = info
      authenticated.value = session.authenticated
      noteNetworkSuccess()

      if (authenticated.value) {
        await loadFiles()
        await loadHistory()
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
      networkFailCount = 2
      online.value = false
      error.value = isTransientNetworkError(cause)
        ? friendlyNetworkMessage()
        : errorMessage(cause)
      startReconnectRetry()
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
      await loadHistory()
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
      abortIntents.clear()
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

  /** 拉取主机侧传输历史（新到旧）。失败时保留上一次快照。 */
  async function loadHistory(): Promise<void> {
    try {
      const response = await getHistory()
      history.value = response.history
      online.value = true
    }
    catch {
      // 瞬时失败：保留现有历史
    }
  }

  /** 清空主机侧传输历史。 */
  async function clearHistory(): Promise<void> {
    try {
      await clearHistoryRequest()
      history.value = []
    }
    catch (cause) {
      handleRequestError(cause)
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
        uploadedBytes: 0,
        speedBytesPerSecond: 0,
        status: tooLarge ? 'error' : 'queued',
        error: tooLarge ? '文件超过服务器大小限制' : undefined,
      })
    }
    scheduleUploads()
  }

  function scheduleUploads(): void {
    while (activeUploads.value < uploadConcurrency.value) {
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
    task.speedBytesPerSecond = 0
    task.etaSeconds = undefined
    task.error = undefined
    activeUploads.value += 1
    const controller = new AbortController()
    uploadControllers.set(task.id, controller)
    const metricsTracker = createTransferMetricsTracker(task.file.size)
    let latestUploadedBytes = 0
    let metricsTimer: ReturnType<typeof setInterval> | undefined

    const refreshMetrics = (): void => {
      const metrics = metricsTracker.update(latestUploadedBytes)
      task.speedBytesPerSecond = metrics.speedBytesPerSecond
      task.etaSeconds = metrics.etaSeconds
    }

    try {
      await runResumableUpload({
        file: task.file,
        sessionID: task.sessionID,
        signal: controller.signal,
        onSession: (sessionID) => {
          task.sessionID = sessionID
        },
        onProgress: (snapshot) => {
          latestUploadedBytes = snapshot.uploadedBytes
          task.uploadedBytes = snapshot.uploadedBytes
          task.progress = snapshot.progress
          task.resumed = snapshot.resumed
          refreshMetrics()
          metricsTimer ??= setInterval(refreshMetrics, 1_000)
        },
      })
      task.status = 'complete'
      task.progress = 100
      task.uploadedBytes = task.file.size
      task.speedBytesPerSecond = 0
      task.etaSeconds = 0
      task.sessionID = undefined
      online.value = true
      await loadFiles()
      await loadHistory()
    }
    catch (cause) {
      if (cause instanceof DOMException && cause.name === 'AbortError') {
        const intent = abortIntents.get(task.id) ?? 'cancel'
        abortIntents.delete(task.id)
        if (intent === 'pause') {
          // 保留 sessionID 与已确认进度，继续时从缺块处恢复。
          task.status = 'paused'
          task.speedBytesPerSecond = 0
          task.etaSeconds = undefined
        }
        else {
          task.status = 'cancelled'
          task.error = undefined
          task.speedBytesPerSecond = 0
          task.etaSeconds = undefined
          await discardUploadSession(task)
          await loadHistory()
        }
      }
      else {
        if (cause instanceof HttpError && cause.status === 401)
          authenticated.value = false
        task.status = 'error'
        task.error = errorMessage(cause)
        task.speedBytesPerSecond = 0
        task.etaSeconds = undefined
        if (!(cause instanceof HttpError))
          online.value = false
        await loadHistory()
      }
    }
    finally {
      if (metricsTimer !== undefined)
        clearInterval(metricsTimer)
      uploadControllers.delete(task.id)
      abortIntents.delete(task.id)
      activeUploads.value = Math.max(0, activeUploads.value - 1)
      scheduleUploads()
    }
  }

  function cancelUpload(id: string): void {
    const task = uploads.value.find(candidate => candidate.id === id)
    if (!task)
      return
    if (task.status === 'queued' || task.status === 'paused') {
      task.status = 'cancelled'
      void discardUploadSession(task)
      void loadHistory()
      return
    }
    if (task.status !== 'uploading')
      return
    abortIntents.set(id, 'cancel')
    task.status = 'cancelled'
    uploadControllers.get(id)?.abort()
  }

  function pauseUpload(id: string): void {
    const task = uploads.value.find(candidate => candidate.id === id)
    if (!task)
      return
    if (task.status === 'queued') {
      task.status = 'paused'
      return
    }
    if (task.status !== 'uploading')
      return
    // 先记录暂停意图并标记 paused，Abort 处理器才能区分「暂停」与「取消」。
    abortIntents.set(id, 'pause')
    task.status = 'paused'
    uploadControllers.get(id)?.abort()
  }

  function resumeUpload(id: string): void {
    const task = uploads.value.find(candidate => candidate.id === id)
    if (!task || task.status !== 'paused')
      return
    task.status = 'queued'
    scheduleUploads()
  }

  function retryUpload(id: string): void {
    const task = uploads.value.find(candidate => candidate.id === id)
    if (!task || !['error', 'cancelled'].includes(task.status))
      return
    task.status = 'queued'
    task.progress = 0
    task.uploadedBytes = 0
    task.speedBytesPerSecond = 0
    task.etaSeconds = undefined
    task.error = undefined
    task.resumed = false
    // 保留 sessionID 以便在服务端会话仍有效时断点续传；会话失效会自动重建。
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

  /** 文本/剪贴板投递：写入接收目录并刷新列表。 */
  async function sendNote(title: string, content: string): Promise<boolean> {
    const body = content.replace(/\s+$/, '')
    if (!body.trim())
      return false
    error.value = ''
    try {
      const response = await createNote({ title: title.trim(), content: body })
      const note = response.file
      files.value = [note, ...files.value.filter(file => file.id !== note.id)]
      return true
    }
    catch (cause) {
      handleRequestError(cause)
      return false
    }
  }

  function connectEvents(): void {
    disconnectEvents()
    events = new EventSource('/api/events')
    events.addEventListener('ready', () => {
      noteNetworkSuccess()
      scheduleLoadFiles(0)
    })
    events.addEventListener('update', () => {
      noteNetworkSuccess()
      // 下载高峰时 files_changed 可能较密，防抖合并
      scheduleLoadFiles(500)
    })
    events.onerror = () => {
      // EventSource 会自动重连；延迟判定离线，避免点下载瞬间闪「重连中」
      if (offlineGraceTimer)
        return
      offlineGraceTimer = window.setTimeout(() => {
        offlineGraceTimer = 0
        if (events) // 仍持有实例说明未手动断开
          noteNetworkFailure(true)
      }, 2500)
    }
    startHeartbeat()
  }

  function scheduleLoadFiles(delayMs: number): void {
    if (loadFilesTimer) {
      window.clearTimeout(loadFilesTimer)
      loadFilesTimer = 0
    }
    loadFilesTimer = window.setTimeout(() => {
      loadFilesTimer = 0
      void loadFiles()
      void loadHistory()
    }, delayMs)
  }

  function startHeartbeat(): void {
    stopHeartbeat()
    heartbeatTimer = window.setInterval(() => {
      if (!authenticated.value)
        return
      void getSession()
        .then((session) => {
          if (session.authenticated)
            noteNetworkSuccess()
        })
        .catch((cause) => {
          if (isTransientNetworkError(cause))
            noteNetworkFailure(false)
        })
    }, 15000)
  }

  function stopHeartbeat(): void {
    if (heartbeatTimer) {
      window.clearInterval(heartbeatTimer)
      heartbeatTimer = 0
    }
  }

  function disconnectEvents(): void {
    events?.close()
    events = null
    stopHeartbeat()
    if (offlineGraceTimer) {
      window.clearTimeout(offlineGraceTimer)
      offlineGraceTimer = 0
    }
    if (loadFilesTimer) {
      window.clearTimeout(loadFilesTimer)
      loadFilesTimer = 0
    }
  }

  function handleRequestError(cause: unknown): void {
    if (cause instanceof HttpError && cause.status === 401) {
      authenticated.value = false
      disconnectEvents()
      error.value = errorMessage(cause)
      return
    }
    if (isTransientNetworkError(cause)) {
      noteNetworkFailure(true)
      return
    }
    error.value = errorMessage(cause)
  }

  onUnmounted(() => {
    disconnectEvents()
    stopHeartbeat()
    stopReconnectRetry()
    if (loadFilesTimer) {
      window.clearTimeout(loadFilesTimer)
      loadFilesTimer = 0
    }
    for (const controller of uploadControllers.values())
      controller.abort()
    abortIntents.clear()
  })

  return {
    serviceInfo: readonly(serviceInfo),
    files: readonly(files),
    uploads: readonly(uploads),
    history: readonly(history),
    authenticated: readonly(authenticated),
    initializing: readonly(initializing),
    pairing: readonly(pairing),
    loadingFiles: readonly(loadingFiles),
    online: readonly(online),
    error: readonly(error),
    isNetworkError: readonly(isNetworkBanner),
    deletingID: readonly(deletingID),
    uploadConcurrency,
    completedUploads,
    totalBytes,
    downloadOrigin,
    initialize,
    pair,
    signOut,
    loadFiles,
    loadHistory,
    clearHistory,
    addFiles,
    cancelUpload,
    pauseUpload,
    resumeUpload,
    retryUpload,
    clearFinishedUploads,
    deleteFile,
    sendNote,
  }
}
