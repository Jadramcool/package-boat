<script setup lang="ts">
import { AlertTriangle, CheckCircle2, LoaderCircle, X } from '@lucide/vue'
import { onMounted } from 'vue'
import AccessQrCard from '@/components/AccessQrCard.vue'
import DesktopFileList from '@/components/DesktopFileList.vue'
import DesktopHeader from '@/components/DesktopHeader.vue'
import StorageSettings from '@/components/StorageSettings.vue'
import WindowTitleBar from '@/components/WindowTitleBar.vue'
import { useDesktopHost } from '@/composables/useDesktopHost'

const desktop = useDesktopHost()
onMounted(desktop.initialize)
</script>

<template>
  <div v-if="desktop.loading.value" class="desktop-boot" aria-live="polite">
    <div class="boot-symbol">L</div>
    <LoaderCircle class="spin" :size="21" />
    <span>正在启动本机服务</span>
  </div>

  <div v-else-if="desktop.state.value" class="desktop-app">
    <WindowTitleBar
      :always-on-top="desktop.alwaysOnTop.value"
      @toggle-always-on-top="desktop.toggleAlwaysOnTop"
    />

    <div class="desktop-shell">
    <Transition name="toast">
      <div v-if="desktop.noticeMessage.value" class="notice-toast" role="status">
        <CheckCircle2 :size="17" />
        <span>{{ desktop.noticeMessage.value }}</span>
      </div>
    </Transition>

    <DesktopHeader
      :device-name="desktop.state.value.settings.device_name"
      :host="desktop.state.value.host"
      :access-url="desktop.accessURL.value"
      :access-url-index="desktop.accessURLIndex.value"
      :busy="desktop.busy.value === 'server'"
      :copied="desktop.copied.value"
      @toggle="desktop.toggleServer"
      @copy="desktop.copyURL"
      @open="desktop.openURL"
      @select-address="desktop.selectAccessURL"
    />

    <main>
      <div v-if="desktop.errorMessage.value || desktop.state.value.host.error" class="error-strip" role="alert">
        <AlertTriangle :size="16" />
        <span>{{ desktop.errorMessage.value || desktop.state.value.host.error }}</span>
        <button v-if="desktop.errorMessage.value" type="button" class="strip-close" aria-label="关闭错误提示" @click="desktop.clearError">
          <X :size="14" />
        </button>
      </div>

      <div class="main-grid">
        <DesktopFileList
          :items="desktop.state.value.items"
          :linked-count="desktop.linkedCount.value"
          :received-count="desktop.receivedCount.value"
          :busy="desktop.busy.value"
          :drag-active="desktop.dragActive.value"
          @refresh="desktop.refresh"
          @reveal="desktop.revealItem"
          @unshare="desktop.unshare"
          @choose="desktop.chooseFiles"
          @clear="desktop.clearSharedFiles"
        />
        <div class="side-panel">
          <AccessQrCard
            v-if="desktop.accessURL.value"
            :url="desktop.accessURL.value"
            :qr-url="desktop.qrAccessURL.value"
            :auto-pair-enabled="desktop.autoPairByQR.value"
            :copied="desktop.copied.value === desktop.accessURL.value"
            @copy="desktop.copyURL"
            @open="desktop.openURL"
            @toggle-auto-pair="desktop.setAutoPairByQR"
          />
          <StorageSettings
            :settings="desktop.state.value.settings"
            :busy="desktop.busy.value === 'directory'"
            @choose-directory="desktop.chooseReceiveDirectory"
          />
        </div>
      </div>
    </main>

    <footer>
      <span>PacketBoat DESKTOP / {{ desktop.state.value.version }}</span>
      <span class="footer-note">文件通过本机局域网直达 · 无云端中转</span>
    </footer>
    </div>
  </div>

  <div v-else class="fatal-state" role="alert">
    <AlertTriangle :size="30" />
    <h1>桌面端未能启动</h1>
    <p>{{ desktop.errorMessage.value }}</p>
  </div>
</template>

<style scoped>
.desktop-app { height: 100dvh; display: flex; flex-direction: column; }
.desktop-shell { flex: 1; min-height: 0; display: flex; flex-direction: column; padding: 24px; overflow: hidden; }
.desktop-shell > main { width: 100%; max-width: 1480px; margin: 0 auto; flex: 1; min-height: 0; overflow-y: auto; }
.desktop-header, footer { width: 100%; max-width: 1480px; margin-right: auto; margin-left: auto; flex: none; }
.error-strip { display: flex; align-items: center; gap: 10px; margin: 18px 0; padding: 13px 16px; border: 1px solid var(--signal); border-radius: 7px; background: var(--signal-soft); color: #6d230e; font-size: 14px; }
.error-strip span { margin-right: auto; }
.strip-close { flex: none; width: 26px; height: 26px; display: grid; place-items: center; border: 0; border-radius: 5px; background: transparent; color: #6d230e; cursor: pointer; }
.strip-close:hover { background: rgb(240 90 50 / 14%); }
.notice-toast { position: fixed; right: 18px; bottom: 74px; z-index: 50; display: flex; align-items: center; gap: 10px; max-width: min(420px, calc(100vw - 36px)); padding: 13px 18px; border: 1px solid rgb(217 255 82 / 35%); border-radius: 10px; background: var(--ink); color: var(--paper); box-shadow: 0 12px 32px rgb(22 27 20 / 35%); font-size: 14px; }
.notice-toast svg { flex: none; color: var(--acid); }
.notice-toast span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.toast-enter-active { transition: opacity .25s ease, transform .25s ease; }
.toast-leave-active { transition: opacity .2s ease, transform .2s ease; }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateY(10px); }
.main-grid { display: grid; grid-template-columns: minmax(0, 1fr) 360px; gap: 24px; align-items: start; margin-top: 24px; }
.desktop-file-list { min-width: 0; }
.side-panel { min-width: 0; display: flex; flex-direction: column; gap: 24px; position: sticky; top: 24px; align-self: start; }
footer { display: flex; justify-content: space-between; flex: none; padding: 16px 4px 0; color: var(--muted); font: 600 12px/1 var(--font-label); letter-spacing: .08em; }
.desktop-boot, .fatal-state { min-height: 100dvh; display: grid; place-content: center; justify-items: center; gap: 13px; background: var(--ink); color: var(--paper); }
.boot-symbol { width: 66px; height: 66px; display: grid; place-items: center; margin-bottom: 10px; border: 1px solid var(--acid); color: var(--acid); font: 750 29px/1 var(--font-display); }
.desktop-boot span { color: rgb(241 238 228 / 62%); font-size: 14px; letter-spacing: .08em; }
.fatal-state h1 { margin: 7px 0 0; font: 700 26px/1 var(--font-display); }
.fatal-state p { max-width: 450px; margin: 0; color: rgb(241 238 228 / 60%); font-size: 14px; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 1100px) {
  .desktop-file-list, .side-panel { grid-column: 1 / -1; }
}
@media (max-width: 900px) {
  .desktop-shell { padding: 14px; }
  footer { gap: 20px; line-height: 1.4; }
}
@media (max-width: 720px) {
  .main-grid { gap: 16px; }
  footer { flex-direction: column; align-items: flex-start; justify-content: flex-start; gap: 10px; font-size: 11px; }
  .error-strip { font-size: 13px; }
}
@media (max-width: 560px) {
  .desktop-shell { padding: 10px; }
  .main-grid { gap: 12px; }
  .footer-note { display: none; }
}
</style>
