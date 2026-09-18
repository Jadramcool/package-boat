import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { shallowRef } from 'vue'

export type UpdaterStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'error'

export function useUpdater() {
  const status = shallowRef<UpdaterStatus>('idle')
  const updateVersion = shallowRef('')
  const progressPercent = shallowRef(0)
  /** 用户可见的检查/安装结果提示（无论成败） */
  const message = shallowRef('')
  let pendingUpdate: Update | null = null
  let messageTimer: number | undefined

  function setMessage(text: string, sticky = false) {
    message.value = text
    if (messageTimer) {
      window.clearTimeout(messageTimer)
      messageTimer = undefined
    }
    // 错误条与新版本条常驻时，toast 可自动消失
    if (text && !sticky) {
      messageTimer = window.setTimeout(() => {
        if (status.value !== 'checking' && status.value !== 'downloading')
          message.value = ''
      }, 4200)
    }
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
      updateVersion.value = pendingUpdate?.version ?? ''
      if (pendingUpdate) {
        status.value = 'available'
        setMessage(`发现新版本 v${updateVersion.value}`)
      }
      else {
        status.value = 'idle'
        setMessage(silent ? '' : '当前已是最新版本')
      }
    }
    catch (error) {
      status.value = 'error'
      setMessage(silent ? '' : '检查更新失败，请检查网络后重试', true)
      if (silent)
        console.warn('检查更新失败（静默模式已忽略）', error)
      else
        console.warn('检查更新失败', error)
    }
  }

  async function install() {
    const update = pendingUpdate
    if (!update || status.value === 'downloading')
      return
    status.value = 'downloading'
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
      setMessage('更新安装失败，请稍后重试', true)
    }
  }

  function dismiss() {
    pendingUpdate = null
    status.value = 'idle'
    setMessage('')
  }

  return {
    status,
    updateVersion,
    progressPercent,
    message,
    checkForUpdate,
    install,
    dismiss,
  }
}
