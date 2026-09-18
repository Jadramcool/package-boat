import { computed, onUnmounted, readonly, shallowRef } from 'vue'
import { type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { ProgressBarStatus, getCurrentWindow } from '@tauri-apps/api/window'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { openUrl } from '@tauri-apps/plugin-opener'
import {
  addLinkedFiles,
  clearSharedFiles as clearSharedFilesCommand,
  chooseLinkedFiles,
  chooseReceiveDirectory as chooseReceiveDirectoryCommand,
  events,
  getState,
  getTransferProgress,
  refreshAccessCode as refreshAccessCodeCommand,
  rescanReceiveDir as rescanReceiveDirCommand,
  revealItem as revealItemCommand,
  setRequirePairing as setRequirePairingCommand,
  setShareReceiveDir as setShareReceiveDirCommand,
  toggleServer as toggleServerCommand,
  unshare as unshareCommand,
} from '@/api'
import type { DesktopState, LocalAddress } from '@/types'

const autoPairPreferenceKey = 'packetboat:auto-pair-by-qr'
const alwaysOnTopPreferenceKey = 'packetboat:always-on-top'

/** 地址列表兜底：老版本后端只给 `urls`，没有 `addresses` 元信息。 */
function addressMetaOf(state: DesktopState | null): readonly LocalAddress[] {
  const host = state?.host
  if (!host) return []
  if (host.addresses.length > 0) return host.addresses
  return host.urls.map(url => {
    const ip = url.replace(/^https?:\/\//, '').replace(/:\d+$/, '')
    return {
      url,
      interface: '',
      ip,
      prefix_len: null,
      virtual_link: false,
      tier: 0,
    } satisfies LocalAddress
  })
}

export function useDesktopHost() {
  const state = shallowRef<DesktopState | null>(null)
  const loading = shallowRef(true)
  const busy = shallowRef('')
  const errorMessage = shallowRef('')
  const noticeMessage = shallowRef('')
  const copied = shallowRef('')
  const selectedAccessURL = shallowRef('')
  const autoPairByQR = shallowRef(loadAutoPairPreference())
  const alwaysOnTop = shallowRef(loadAlwaysOnTopPreference())
  const dragActive = shallowRef(false)
  let cancelStateEvent: UnlistenFn | undefined
  let cancelErrorEvent: UnlistenFn | undefined
  let cancelNoticeEvent: UnlistenFn | undefined
  let cancelDragDrop: UnlistenFn | undefined
  let copyTimer = 0
  let noticeTimer = 0
  let progressTimer = 0

  const linkedCount = computed(() => state.value?.items.filter(item => item.source_type === 'linked').length ?? 0)
  const receivedCount = computed(() => state.value?.items.filter(item => item.source_type === 'received').length ?? 0)
  const inboxCount = computed(() => state.value?.items.filter(item => item.source_type === 'inbox').length ?? 0)

  /** 候选地址（带可达性元信息），已由后端按可达性排序。 */
  const addresses = computed<readonly LocalAddress[]>(() => addressMetaOf(state.value))
  /** 首选地址：可达性最高的一条（tier 最小，后端已排好序）。 */
  const primaryAddress = computed(() => addresses.value[0] ?? null)
  /** 当前选中的地址元信息；选中项失效时回落到首选地址。 */
  const activeAddress = computed(() =>
    addresses.value.find(a => a.url === accessURL.value) ?? primaryAddress.value,
  )
  /**
   * 明确不可达（虚拟网卡 / 点对点链路）：需要给用户黄色警示。
   * 仅在存在更优候选时告警，避免全机只有虚拟网卡时满屏红字。
   */
  const addressUnreachable = computed(() => {
    const active = activeAddress.value
    if (!active || addresses.value.length < 2) return false
    return active.tier >= 2
  })
  /** 除当前地址外还有别的候选（决定是否显示地址切换器）。 */
  const hasAlternateAddresses = computed(() =>
    addresses.value.filter(a => a.tier < 2).length > 1 || addresses.value.length > 1,
  )

  const accessURL = computed(() => {
    const urls = state.value?.host.urls ?? []
    return urls.includes(selectedAccessURL.value) ? selectedAccessURL.value : (urls[0] ?? '')
  })
  const accessURLIndex = computed(() => Math.max(0, state.value?.host.urls.indexOf(accessURL.value) ?? 0))
  /** 是否需要配对码访问（服务端设置）；旧后端无此字段时按开启处理。 */
  const requirePairing = computed(() => state.value?.settings.require_pairing ?? true)
  /** 是否把接收目录既有文件列入共享清单。 */
  const shareReceiveDir = computed(() => state.value?.settings.share_receive_dir ?? false)
  const qrAccessURL = computed(() => {
    const url = accessURL.value
    const code = state.value?.host.access_code.replace(/\D/g, '') ?? ''
    if (!url)
      return url
    // 免配对模式下二维码不需要携带配对码
    if (!requirePairing.value)
      return url
    if (!autoPairByQR.value || code.length !== 6)
      return url
    return `${url.replace(/#.*$/, '')}#code=${encodeURIComponent(code)}`
  })

  async function initialize() {
    loading.value = true
    // 窗口置顶校准：与本地偏好对齐（失败时保留偏好值，不阻塞启动）
    void syncAlwaysOnTop()
    // 任务栏进度轮询：独立启动，确保即使事件监听失败也不会停摆
    progressTimer = window.setInterval(() => void syncTaskbarProgress(), 500)
    try {
      await refresh()
      // tauri-specta 类型安全事件：事件名与载荷由 Rust 侧 bindings 同步
      cancelStateEvent = await events.stateChanged.listen(() => void refresh())
      cancelErrorEvent = await events.errorNotice.listen((event) => {
        errorMessage.value = typeof event.payload === 'string' ? event.payload : '服务端发生错误，请查看日志'
        void refresh()
      })
      cancelNoticeEvent = await events.successNotice.listen((event) => {
        noticeMessage.value = typeof event.payload === 'string' ? event.payload : ''
        window.clearTimeout(noticeTimer)
        noticeTimer = window.setTimeout(() => noticeMessage.value = '', 3200)
      })
      cancelDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === 'drop') {
          dragActive.value = false
          if (event.payload.paths.length > 0)
            void addDroppedFiles(event.payload.paths)
        }
        else if (event.payload.type === 'enter' || event.payload.type === 'over') {
          dragActive.value = true
        }
        else {
          dragActive.value = false
        }
      })
    }
    catch (error) {
      errorMessage.value = messageFrom(error)
    }
    finally {
      loading.value = false
    }
  }

  async function syncTaskbarProgress() {
    try {
      const progress = await getTransferProgress()
      if (!progress.active) {
        await getCurrentWindow().setProgressBar({ status: ProgressBarStatus.None })
        return
      }
      if (progress.total && progress.total > 0) {
        // Windows 任务栏进度值范围为 0-100（SetProgressValue 的 nCompleted）
        const percent = Math.min(100, Math.round((progress.done / progress.total) * 100))
        await getCurrentWindow().setProgressBar({
          status: ProgressBarStatus.Normal,
          progress: percent,
        })
      }
      else {
        await getCurrentWindow().setProgressBar({ status: ProgressBarStatus.Indeterminate })
      }
    }
    catch {
      // 轮询失败忽略（窗口可能已关闭）
    }
  }

  async function refresh() {
    state.value = await getState()
  }

  async function chooseFiles() {
    await perform('files', async () => {
      const added = await chooseLinkedFiles()
      showAddedNotice(added.length)
    })
  }

  async function addDroppedFiles(paths: string[]) {
    await perform('files', async () => {
      const added = await addLinkedFiles(paths)
      showAddedNotice(added.length)
    })
  }

  function showAddedNotice(count: number) {
    showNotice(count > 0
      ? `已添加 ${count} 个共享文件`
      : '所选文件已经在共享清单中。')
  }

  function showNotice(message: string) {
    noticeMessage.value = message
    window.clearTimeout(noticeTimer)
    noticeTimer = window.setTimeout(() => noticeMessage.value = '', 3200)
  }

  async function unshare(id: string) {
    await perform(`unshare:${id}`, () => unshareCommand(id))
  }

  async function clearSharedFiles() {
    const count = state.value?.items.length ?? 0
    if (count === 0 || busy.value)
      return
    if (!window.confirm(`确定清空 ${count} 个共享条目吗？\n\n只移除共享清单记录，不删除原文件或接收目录中的文件。`))
      return

    await perform('clear', async () => {
      const cleared = await clearSharedFilesCommand()
      showNotice(`已清空 ${cleared} 个共享条目，文件未删除`)
    })
  }

  async function chooseReceiveDirectory() {
    await perform('directory', async () => {
      await chooseReceiveDirectoryCommand()
    })
  }

  async function toggleServer() {
    await perform('server', () => toggleServerCommand())
  }

  /** 切换「是否需要配对码访问」；后端写设置并在服务运行中时重启。 */
  async function setPairingRequired(enabled: boolean) {
    await perform('pairing', () => setRequirePairingCommand(enabled))
  }

  /** 切换「是否展示接收目录内既有文件」。 */
  async function setShareReceiveDirEnabled(enabled: boolean) {
    await perform('share-receive-dir', () => setShareReceiveDirCommand(enabled))
  }

  /** 独立刷新：重新扫描接收目录，恢复已移出清单的本地文件。 */
  async function rescanReceiveDirectory() {
    busy.value = 'rescan-inbox'
    errorMessage.value = ''
    try {
      const restored = await rescanReceiveDirCommand()
      await refresh()
      showNotice(restored > 0 ? `已恢复 ${restored} 个接收目录条目` : '扫描完成：没有需要恢复的条目')
    }
    catch (error) {
      errorMessage.value = messageFrom(error)
    }
    finally {
      busy.value = ''
    }
  }

  /** 手动刷新配对码（不重启服务）。 */
  async function refreshAccessCode() {
    busy.value = 'access-code'
    errorMessage.value = ''
    try {
      await refreshAccessCodeCommand()
      await refresh()
    }
    catch (error) {
      errorMessage.value = messageFrom(error)
    }
    finally {
      busy.value = ''
    }
  }

  async function revealItem(id: string) {
    await perform(`reveal:${id}`, () => revealItemCommand(id))
  }

  async function copyURL(url: string) {
    try {
      await writeText(url)
      copied.value = url
      window.clearTimeout(copyTimer)
      window.clearTimeout(noticeTimer)
      copyTimer = window.setTimeout(() => copied.value = '', 1800)
    }
    catch (error) {
      errorMessage.value = messageFrom(error)
    }
  }

  async function openURL(url: string) {
    try {
      await openUrl(url)
    }
    catch (error) {
      errorMessage.value = messageFrom(error)
    }
  }

  function selectAccessURL(url: string) {
    if (state.value?.host.urls.includes(url))
      selectedAccessURL.value = url
  }

  async function syncAlwaysOnTop() {
    try {
      alwaysOnTop.value = await getCurrentWindow().isAlwaysOnTop()
    }
    catch {
      // 校准失败时保留本地偏好值，不影响启动流程
    }
  }

  async function toggleAlwaysOnTop() {
    const next = !alwaysOnTop.value
    try {
      await getCurrentWindow().setAlwaysOnTop(next)
      alwaysOnTop.value = next
      try {
        window.localStorage.setItem(alwaysOnTopPreferenceKey, String(next))
      }
      catch {
        // 存储不可用时忽略：本次会话仍生效
      }
    }
    catch (error) {
      errorMessage.value = messageFrom(error)
    }
  }

  function setAutoPairByQR(enabled: boolean) {
    autoPairByQR.value = enabled
    try {
      window.localStorage.setItem(autoPairPreferenceKey, String(enabled))
    }
    catch {
      // 开关在当前会话仍然生效；存储不可用时忽略。
    }
  }

  async function perform(key: string, action: () => Promise<unknown>) {
    busy.value = key
    errorMessage.value = ''
    try {
      await action()
      await refresh()
    }
    catch (error) {
      errorMessage.value = messageFrom(error)
    }
    finally {
      busy.value = ''
    }
  }

  /** 关闭当前错误提示（仅清本地错误；服务端 host.error 由状态快照决定）。 */
  function clearError() {
    errorMessage.value = ''
  }

  onUnmounted(() => {
    cancelStateEvent?.()
    cancelErrorEvent?.()
    cancelNoticeEvent?.()
    cancelDragDrop?.()
    window.clearTimeout(copyTimer)
    window.clearTimeout(noticeTimer)
    window.clearInterval(progressTimer)
  })

  return {
    state: readonly(state),
    loading: readonly(loading),
    busy: readonly(busy),
    errorMessage: readonly(errorMessage),
    noticeMessage: readonly(noticeMessage),
    copied: readonly(copied),
    selectedAccessURL: readonly(selectedAccessURL),
    autoPairByQR: readonly(autoPairByQR),
    alwaysOnTop: readonly(alwaysOnTop),
    dragActive: readonly(dragActive),
    addresses,
    primaryAddress,
    activeAddress,
    addressUnreachable,
    hasAlternateAddresses,
    accessURL,
    accessURLIndex,
    qrAccessURL,
    linkedCount,
    receivedCount,
    inboxCount,
    requirePairing,
    shareReceiveDir,
    initialize,
    refresh,
    chooseFiles,
    addDroppedFiles,
    unshare,
    clearSharedFiles,
    chooseReceiveDirectory,
    toggleServer,
    setPairingRequired,
    setShareReceiveDirEnabled,
    rescanReceiveDirectory,
    refreshAccessCode,
    revealItem,
    copyURL,
    openURL,
    selectAccessURL,
    setAutoPairByQR,
    toggleAlwaysOnTop,
    clearError,
  }
}

function loadAutoPairPreference(): boolean {
  try {
    return window.localStorage.getItem(autoPairPreferenceKey) !== 'false'
  }
  catch {
    return true
  }
}

function loadAlwaysOnTopPreference(): boolean {
  try {
    return window.localStorage.getItem(alwaysOnTopPreferenceKey) === 'true'
  }
  catch {
    return false
  }
}

function messageFrom(error: unknown): string {
  return error instanceof Error ? error.message : '操作失败，请稍后重试'
}
