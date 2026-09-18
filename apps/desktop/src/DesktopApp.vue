<script setup lang="ts">
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  LoaderCircle,
  X,
} from "@lucide/vue";
import { onMounted } from "vue";
import ConnectionSettings from "@/components/ConnectionSettings.vue";
import DesktopFileList from "@/components/DesktopFileList.vue";
import DesktopHeader from "@/components/DesktopHeader.vue";
import StorageSettings from "@/components/StorageSettings.vue";
import WindowTitleBar from "@/components/WindowTitleBar.vue";
import BrandMark from "@/components/BrandMark.vue";
import { useDesktopHost } from "@/composables/useDesktopHost";
import { useUpdater } from "@/composables/useUpdater";

const desktop = useDesktopHost();
const updater = useUpdater();
onMounted(() => {
  desktop.initialize();
  updater.checkForUpdate();
});
</script>

<template>
  <div v-if="desktop.loading.value" class="desktop-boot" aria-live="polite">
    <div class="boot-symbol" aria-hidden="true"><BrandMark :size="40" /></div>
    <LoaderCircle class="spin" :size="21" />
    <span>正在启动本机服务</span>
  </div>

  <div v-else-if="desktop.state.value" class="desktop-app">
    <WindowTitleBar
      :always-on-top="desktop.alwaysOnTop.value"
      @toggle-always-on-top="desktop.toggleAlwaysOnTop" />

    <!-- 命令条放在 shell 之外：通栏深色，与标题栏连成一片，不占用内容区左右留白。
         二维码已移到右侧「扫码接入」卡常驻，命令条不再承担扫码入口。 -->
    <DesktopHeader
      :device-name="desktop.state.value.settings.device_name"
      :host="desktop.state.value.host"
      :access-url="desktop.accessURL.value"
      :active-address="desktop.activeAddress.value"
      :address-unreachable="desktop.addressUnreachable.value"
      :busy="desktop.busy.value === 'server'"
      :refreshing-code="desktop.busy.value === 'access-code'"
      :copied="desktop.copied.value"
      @toggle="desktop.toggleServer"
      @copy="desktop.copyURL"
      @open="desktop.openURL"
      @refresh-code="desktop.refreshAccessCode" />

    <div class="desktop-shell">
      <Transition name="toast">
        <div
          v-if="desktop.noticeMessage.value"
          class="notice-toast"
          role="status">
          <CheckCircle2 :size="17" />
          <span>{{ desktop.noticeMessage.value }}</span>
        </div>
      </Transition>

      <main>
        <div
          v-if="
            updater.status.value === 'available' ||
            updater.status.value === 'downloading'
          "
          class="update-strip"
          role="status">
          <Download :size="16" />
          <span>发现新版本 v{{ updater.updateVersion.value }}</span>
          <button
            type="button"
            class="update-install"
            :disabled="updater.status.value === 'downloading'"
            @click="updater.install">
            <span>
              {{
                updater.status.value === "downloading"
                  ? `下载中 ${updater.progressPercent.value}%`
                  : "安装并重启"
              }}
            </span>
          </button>
          <button
            v-if="updater.status.value === 'available'"
            type="button"
            class="strip-close"
            aria-label="忽略此更新"
            @click="updater.dismiss">
            <X :size="14" />
          </button>
        </div>

        <div
          v-else-if="updater.status.value === 'error'"
          class="error-strip"
          role="alert">
          <AlertTriangle :size="16" />
          <span>自动更新检查失败，可稍后重试</span>
          <button
            type="button"
            class="strip-retry"
            @click="updater.checkForUpdate(false)">
            <span>重试</span>
          </button>
          <button
            type="button"
            class="strip-close"
            aria-label="关闭更新提示"
            @click="updater.dismiss">
            <X :size="14" />
          </button>
        </div>

        <div
          v-if="desktop.errorMessage.value || desktop.state.value.host.error"
          class="error-strip"
          role="alert">
          <AlertTriangle :size="16" />
          <span>{{
            desktop.errorMessage.value || desktop.state.value.host.error
          }}</span>
          <button
            v-if="desktop.errorMessage.value"
            type="button"
            class="strip-close"
            aria-label="关闭错误提示"
            @click="desktop.clearError">
            <X :size="14" />
          </button>
        </div>

        <div class="main-grid">
          <DesktopFileList
            :items="desktop.state.value.items"
            :busy="desktop.busy.value"
            :drag-active="desktop.dragActive.value"
            :linked-count="desktop.linkedCount.value"
            :received-count="desktop.receivedCount.value"
            :inbox-count="desktop.inboxCount.value"
            @refresh="desktop.refresh"
            @reveal="desktop.revealItem"
            @unshare="desktop.unshare"
            @choose="desktop.chooseFiles"
            @clear="desktop.clearSharedFiles" />

          <!-- 低频设置常驻右列：竖着住，纵向一分不占，宽屏不折叠。 -->
          <aside class="side-panel" aria-label="设置">
            <ConnectionSettings
              :addresses="desktop.addresses.value"
              :active-address="desktop.activeAddress.value"
              :access-url="desktop.accessURL.value"
              :access-url-index="desktop.accessURLIndex.value"
              :address-unreachable="desktop.addressUnreachable.value"
              :has-alternate-addresses="desktop.hasAlternateAddresses.value"
              :auto-pair-enabled="desktop.autoPairByQR.value"
              :pairing-required="desktop.requirePairing.value"
              :running="desktop.state.value.host.running"
              :qr-url="desktop.qrAccessURL.value"
              @select-address="desktop.selectAccessURL"
              @toggle-auto-pair="desktop.setAutoPairByQR"
              @toggle-pairing="desktop.setPairingRequired" />
            <StorageSettings
              :settings="desktop.state.value.settings"
              :busy="desktop.busy.value === 'directory'"
              :rescanning-inbox="desktop.busy.value === 'rescan-inbox'"
              @choose-directory="desktop.chooseReceiveDirectory"
              @toggle-share-receive-dir="desktop.setShareReceiveDirEnabled"
              @rescan-receive-dir="desktop.rescanReceiveDirectory" />
          </aside>
        </div>
      </main>

      <footer>
        <div class="footer-inner">
          <span class="footer-product"
            >PacketBoat 桌面端 · v{{ desktop.state.value.version }}</span
          >
          <span class="footer-dot" aria-hidden="true" />
          <span class="footer-note">文件通过本机局域网直达 · 无云端中转</span>
          <span class="footer-spacer" />
          <button
            type="button"
            class="footer-action"
            :disabled="
              updater.status.value === 'checking' ||
              updater.status.value === 'downloading'
            "
            @click="updater.checkForUpdate(false)">
            {{ updater.status.value === "checking" ? "正在检查…" : "检查更新" }}
          </button>
        </div>
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
.desktop-app {
  height: 100dvh;
  display: flex;
  flex-direction: column;
}
/* 命令条通栏后，shell 只需承担内容区的呼吸感——14px 与列间距、卡内边距同一节奏 */
/* shell 不给 bottom padding：页脚必须贴窗口底边，否则会被垫高 */
.desktop-shell {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 14px 14px 0;
  overflow: hidden;
}
/* 内容区不整页滚动：提示条常驻，左右栏各自 overflow */
.desktop-shell > main {
  width: 100%;
  max-width: 1480px;
  margin: 0 auto;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding-bottom: 14px;
}
/* 页脚方案 A：文案左成组，操作在列表同宽的右端 */
footer {
  width: 100%;
  flex: none;
  border-top: 1px solid var(--line);
  background: var(--dock-0);
}
.footer-inner {
  width: 100%;
  max-width: 1480px;
  margin: 0 auto;
  padding: 12px;
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--muted);
  font: 600 12.5px/1.45 var(--font-label);
  letter-spacing: 0.06em;
}
.footer-product,
.footer-note {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.footer-dot {
  width: 3px;
  height: 3px;
  flex: none;
  border-radius: 50%;
  background: var(--line-strong);
}
.footer-spacer {
  flex: 1;
}
.error-strip {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0 0 14px;
  padding: 14px 16px;
  border: 1px solid var(--signal);
  border-radius: 10px;
  background: var(--signal-soft);
  color: var(--signal-deep);
  font-size: 14px;
}
.error-strip span {
  margin-right: auto;
}
.error-strip > svg,
.update-strip > svg,
.notice-toast svg {
  flex: none;
  display: block;
}
.notice-toast svg {
  color: var(--acid);
}
.strip-close {
  flex: none;
  width: 26px;
  height: 26px;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--signal-deep);
  cursor: pointer;
}
.strip-close:hover {
  background: rgb(240 90 50 / 14%);
}
.strip-retry {
  flex: none;
  min-height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 6px 14px;
  border: 1px solid var(--signal-deep);
  border-radius: 6px;
  background: transparent;
  color: var(--signal-deep);
  font: 700 13px/1.2 var(--font-label);
  cursor: pointer;
}
.strip-retry:hover:not(:disabled) {
  background: rgb(239 90 50 / 14%);
}
.strip-retry:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.update-strip {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0 0 14px;
  padding: 14px 16px;
  border: 1px solid color-mix(in srgb, var(--acid-deep) 40%, transparent);
  border-radius: 10px;
  background: var(--acid-wash);
  color: var(--ink);
  font-size: 14px;
}
.update-strip span {
  margin-right: auto;
}
.update-install {
  flex: none;
  min-height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 7px 14px;
  border: 1px solid var(--ink);
  border-radius: 6px;
  background: var(--ink);
  color: var(--acid);
  font: 700 13px/1.2 var(--font-label);
  letter-spacing: 0.02em;
  cursor: pointer;
}
.update-install:disabled {
  opacity: 0.6;
  cursor: default;
}
.notice-toast {
  position: fixed;
  right: 18px;
  bottom: 74px;
  z-index: 50;
  display: flex;
  align-items: center;
  gap: 10px;
  max-width: min(420px, calc(100vw - 36px));
  padding: 13px 18px;
  border: 1px solid rgb(215 249 84 / 35%);
  border-radius: 12px;
  background: var(--ink);
  color: var(--paper);
  box-shadow: 0 12px 32px rgb(31 39 28 / 35%);
  font-size: 14px;
}
.notice-toast svg {
  flex: none;
  color: var(--acid);
}
.notice-toast span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toast-enter-active {
  transition:
    opacity 0.25s ease,
    transform 0.25s ease;
}
.toast-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
/* 清单在上、设置常驻右列。宽度 PB-UI-03 定的 296px → 336px（用户反馈地址下拉
   与接收路径框被挤压截断）：+40px 后下拉能完整显示「局域网 · x.x.x.x (网卡 /掩码)」，
   1280 视口下主列仍有 902px，五列网格的 fr 份额全部高于下限，清单不受挤压。
   左右栏在网格内各自滚动：main 不滚，滚轮落在哪列就滚哪列。 */
.main-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 336px;
  gap: 14px;
  align-items: stretch;
}
.desktop-file-list {
  min-width: 0;
  min-height: 0;
  height: 100%;
}
.side-panel {
  min-width: 0;
  min-height: 0;
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
  overscroll-behavior: contain;
}
/* 卡片保持自然高度：空间不够时整栏滚动，禁止 flex 把卡压扁 */
.side-panel > * {
  flex: none;
}
.side-panel::-webkit-scrollbar {
  width: 6px;
}
.footer-action {
  flex: none;
  min-height: 30px;
  padding: 6px 12px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--line-strong);
  border-radius: 6px;
  background: transparent;
  color: var(--muted);
  font: 600 12px/1.2 var(--font-label);
  letter-spacing: 0.06em;
  cursor: pointer;
  transition:
    color 0.15s,
    border-color 0.15s;
}
.footer-action:hover:not(:disabled) {
  color: var(--ink);
  border-color: var(--ink);
}
.footer-action:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.desktop-boot,
.fatal-state {
  min-height: 100dvh;
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 13px;
  background: var(--ink);
  color: var(--paper);
}
.boot-symbol {
  width: 66px;
  height: 66px;
  display: grid;
  place-items: center;
  margin-bottom: 10px;
  border: 1px solid var(--acid);
  border-radius: 14px;
  color: var(--acid);
}
.desktop-boot span {
  color: rgb(251 252 247 / 62%);
  font-size: 14px;
  letter-spacing: 0.08em;
}
.fatal-state h1 {
  margin: 7px 0 0;
  font: 700 26px/1 var(--font-display);
}
.fatal-state p {
  max-width: 450px;
  margin: 0;
  color: rgb(251 252 247 / 60%);
  font-size: 14px;
}
.spin {
  animation: spin 1s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
/* —— 窄于 1100px：右列落到清单下方。——
   这不是「另一套设计」，只是同一批设置的降级排布；宽屏不折叠的承诺在此之上保持不变。 */
@media (max-width: 1100px) {
  .main-grid {
    grid-template-columns: minmax(0, 1fr);
  }
  .side-panel {
    position: static;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  }
}
@media (max-width: 900px) {
  .desktop-shell {
    padding: 12px 12px 0;
  }
  .footer-inner {
    gap: 20px;
    line-height: 1.4;
    padding: 12px 12px 18px;
  }
}
@media (max-width: 780px) {
  .main-grid {
    gap: 12px;
  }
  .side-panel {
    gap: 12px;
  }
  .footer-inner {
    flex-wrap: wrap;
    gap: 8px;
    font-size: 11.5px;
  }
  .footer-dot {
    display: none;
  }
  .footer-spacer {
    display: none;
  }
  .footer-action {
    margin-left: auto;
  }
  .error-strip {
    font-size: 13px;
  }
}
@media (max-width: 560px) {
  .main-grid {
    gap: 10px;
  }
  .footer-note {
    display: none;
  }
}
</style>

