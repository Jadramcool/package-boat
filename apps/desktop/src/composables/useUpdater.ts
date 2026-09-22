import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { computed, shallowRef } from 'vue'

export type UpdaterStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'error'

export function useUpdater() {
  const status = shallowRef<UpdaterStatus>('idle')
  const updateVersion = shallowRef('')
  const currentVersion = shallowRef('')
  const updateDate = shallowRef('')
  /** 更新说明（release notes / latest.json notes） */
  const updateNotes = shallowRef('')
  const progressPercent = shallowRef(0)
  /** 用户可见的检查/安装结果提示（无论成败） */
  const message = shallowRef('')
  /** 是否弹出更新信息对话框 */
  const dialogOpen = shallowRef(false)
  let pendingUpdate: Update | null = null
  let messageTimer: number | undefined

  const hasNotes = computed(() => updateNotes.value.trim().length > 0)

  function setMessage(text: string, sticky = false) {
    message.value = text
    if (messageTimer) {
      window.clearTimeout(messageTimer)
      messageTimer = undefined
    }
    if (text && !sticky) {
      messageTimer = window.setTimeout(() => {
        if (status.value !== 'checking' && status.value !== 'downloading') {
          message.value = ''
          // toast 消失后错误态复位，避免页脚按钮状态粘住
          if (status.value === 'error')
            status.value = 'idle'
        }
      }, 4200)
    }
  }

  function applyUpdateMeta(update: Update | null) {
    updateVersion.value = update?.version ?? ''
    currentVersion.value = update?.currentVersion ?? ''
    updateDate.value = update?.date ?? ''
    const raw = update?.body?.trim() ?? ''
    // 无实质内容时避免展示占位文案
    const placeholder = /^PacketBoat\s+v?\d+\.\d+\.\d+\s*$/i
    updateNotes.value = placeholder.test(raw) ? '' : raw
  }

  // silent：启动时静默检查——有新版本/失败仍提示；无更新不打扰
  // 非 silent（手动「检查更新」）：成功失败都必须提示
  async function checkForUpdate(silent = true) {
    if (status.value === 'checking' || status.value === 'downloading')
      return
    status.value = 'checking'
    if (!silent)
      setMessage('正在检查更新…', true)
    try {
      pendingUpdate = await check()
      applyUpdateMeta(pendingUpdate)
      if (pendingUpdate) {
        status.value = 'available'
        dialogOpen.value = true
        setMessage(`发现新版本 v${updateVersion.value}`)
      }
      else {
        status.value = 'idle'
        dialogOpen.value = false
        setMessage(silent ? '' : '当前已是最新版本')
      }
    }
    catch (error) {
      status.value = 'error'
      dialogOpen.value = false
      // 失败只走 toast，不占用页面横幅；自动消失
      setMessage('检查更新失败，请检查网络后重试', false)
      if (silent)
        console.warn('检查更新失败（静默模式）', error)
      else
        console.warn('检查更新失败', error)
    }
  }

  async function install() {
    const update = pendingUpdate
    if (!update || status.value === 'downloading')
      return
    status.value = 'downloading'
    dialogOpen.value = true
    progressPercent.value = 0
    setMessage(`正在下载 v${update.version}…`, true)
    try {
      let received = 0
      let total = 0
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          total = event.data.contentLength ?? 0
        }
        else if (event.event === 'Progress') {
          received += event.data.chunkLength
          if (total > 0)
            progressPercent.value = Math.round((received / total) * 100)
        }
      })
      setMessage('更新完成，正在重启…', true)
      await relaunch()
    }
    catch (error) {
      console.error('更新安装失败', error)
      status.value = 'error'
      setMessage('更新安装失败，请稍后重试', false)
    }
  }

  function openDialog() {
    if (status.value === 'available' || status.value === 'downloading')
      dialogOpen.value = true
  }

  function dismissDialog() {
    // 下载中不允许关闭弹窗，避免误触
    if (status.value === 'downloading')
      return
    dialogOpen.value = false
  }

  function dismiss() {
    pendingUpdate = null
    status.value = 'idle'
    dialogOpen.value = false
    setMessage('')
  }

  return {
    status,
    updateVersion,
    currentVersion,
    updateDate,
    updateNotes,
    hasNotes,
    progressPercent,
    message,
    dialogOpen,
    checkForUpdate,
    install,
    openDialog,
    dismissDialog,
    dismiss,
  }
}
