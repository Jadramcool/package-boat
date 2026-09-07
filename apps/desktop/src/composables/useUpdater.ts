import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { shallowRef } from 'vue'

export type UpdaterStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'error'

export function useUpdater() {
  const status = shallowRef<UpdaterStatus>('idle')
  const updateVersion = shallowRef('')
  const progressPercent = shallowRef(0)
  let pendingUpdate: Update | null = null

  // 启动时的静默检查：endpoint 未配置或离线时不打扰用户；手动重试才提示错误
  async function checkForUpdate(silent = true) {
    if (status.value === 'checking' || status.value === 'downloading')
      return
    status.value = 'checking'
    try {
      pendingUpdate = await check()
      updateVersion.value = pendingUpdate?.version ?? ''
      status.value = pendingUpdate ? 'available' : 'idle'
    }
    catch (error) {
      status.value = silent ? 'idle' : 'error'
      if (silent) {
        console.warn('检查更新失败（静默模式已忽略）', error)
      }
    }
  }

  async function install() {
    const update = pendingUpdate
    if (!update || status.value === 'downloading')
      return
    status.value = 'downloading'
    progressPercent.value = 0
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
      await relaunch()
    }
    catch (error) {
      console.error('更新安装失败', error)
      status.value = 'error'
    }
  }

  function dismiss() {
    pendingUpdate = null
    status.value = 'idle'
  }

  return {
    status,
    updateVersion,
    progressPercent,
    checkForUpdate,
    install,
    dismiss,
  }
}
