<script setup lang="ts">
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  LoaderCircle,
  X,
} from "@lucide/vue";
import { onMounted, onUnmounted } from "vue";
import ConnectionSettings from "@/components/ConnectionSettings.vue";
import ConnectedDevices from "@/components/ConnectedDevices.vue";
import ActiveTransfers from "@/components/ActiveTransfers.vue";
import DesktopFileList from "@/components/DesktopFileList.vue";
import DesktopHeader from "@/components/DesktopHeader.vue";
import StorageSettings from "@/components/StorageSettings.vue";
import WindowTitleBar from "@/components/WindowTitleBar.vue";
import BrandMark from "@/components/BrandMark.vue";
import { useDesktopHost } from "@/composables/useDesktopHost";
import { useUpdater } from "@/composables/useUpdater";

const desktop = useDesktopHost();
const updater = useUpdater();

function formatUpdateDate(value: string): string {
  const raw = value.trim();
  if (!raw) return "";
  const d = new Date(raw);
  if (Number.isNaN(d.getTime())) return raw;
  return d.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

/** 桌面端：锁定页面级滚动，仅内容区内部滚动（顶栏/页脚固定）。 */
function lockPageScroll(): void {
  const html = document.documentElement
  const body = document.body
  html.style.overflow = 'hidden'
  html.style.height = '100%'
  body.style.overflow = 'hidden'
  body.style.height = '100%'
  const app = document.getElementById('app')
  if (app) {
    app.style.height = '100%'
    app.style.overflow = 'hidden'
  }
}

function unlockPageScroll(): void {
  const html = document.documentElement
  const body = document.body
  html.style.overflow = ''
  html.style.height = ''
  body.style.overflow = ''
  body.style.height = ''
  const app = document.getElementById('app')
  if (app) {
    app.style.height = ''
    app.style.overflow = ''
  }
}

onMounted(() => {
  lockPageScroll();
  desktop.initialize();
  updater.checkForUpdate();
});

onUnmounted(() => {
  unlockPageScroll();
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
      <div class="toast-stack">
      <Transition name="toast">
        <div
          v-if="updater.message.value"
          class="notice-toast"
          :class="{ warn: updater.status.value === 'error' }"
          role="status">
          <CheckCircle2 v-if="updater.status.value !== 'error'" :size="17" />
          <AlertTriangle v-else :size="17" />
          <span>{{ updater.message.value }}</span>
        </div>
      </Transition>
      <Transition name="toast">
        <div
          v-if="desktop.noticeMessage.value"
          class="notice-toast"
          role="status">
          <CheckCircle2 :size="17" />
          <span>{{ desktop.noticeMessage.value }}</span>
        </div>
      </Transition>
      </div>

      <main>
        <!-- 更新检查/安装失败：仅 toast（见 toast-stack），不占用内容区横幅 -->

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
            <ActiveTransfers
              :transfers="desktop.transfers.value"
              :devices="desktop.devices.value"
              :running="desktop.state.value.host.running"
              @cancel="desktop.cancelTransfer"
              @pause="desktop.pauseTransfer"
              @resume="desktop.resumeTransfer" />
            <ConnectedDevices
              :devices="desktop.devices.value"
              :running="desktop.state.value.host.running" />
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
            v-if="updater.status.value === 'available'"
            type="button"
            class="footer-action"
            @click="updater.openDialog">
            可更新至 v{{ updater.updateVersion.value }}
          </button>
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

    <!-- 更新信息弹窗：展示版本号 / 更新说明 / 下载进度 -->
    <div
      v-if="updater.dialogOpen.value"
      class="update-dialog-backdrop"
      role="presentation"
      @click.self="updater.dismissDialog">
      <section
        class="update-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="update-dialog-title">
        <header class="update-dialog-head">
          <div class="update-dialog-icon" aria-hidden="true">
            <Download :size="20" />
          </div>
          <div class="update-dialog-titles">
            <h2 id="update-dialog-title">
              发现新版本 v{{ updater.updateVersion.value }}
            </h2>
            <p class="update-dialog-meta">
              <span v-if="updater.currentVersion.value">
                当前 v{{ updater.currentVersion.value }}
              </span>
              <span
                v-if="updater.currentVersion.value && updater.updateDate.value"
                aria-hidden="true"
                >·</span
              >
              <span v-if="updater.updateDate.value">{{
                formatUpdateDate(updater.updateDate.value)
              }}</span>
            </p>
          </div>
          <button
            v-if="updater.status.value === 'available'"
            type="button"
            class="update-dialog-close"
            aria-label="稍后再说"
            @click="updater.dismissDialog">
            <X :size="16" />
          </button>
        </header>

        <div class="update-dialog-body">
          <p v-if="!updater.hasNotes.value" class="update-notes-empty">
            安装包已就绪，可立即下载并重启应用完成更新。
          </p>
          <div v-else class="update-notes" role="region" aria-label="更新说明">
            <div class="update-notes-label">更新说明</div>
            <div class="update-notes-content">
              {{ updater.updateNotes.value }}
            </div>
          </div>
        </div>

        <div v-if="updater.status.value === 'downloading'" class="update-progress">
          <div class="update-progress-track">
            <div
              class="update-progress-bar"
              :style="{ width: `${updater.progressPercent.value}%` }" />
          </div>
          <span class="update-progress-text"
            >下载中 {{ updater.progressPercent.value }}%</span
          >
        </div>

        <footer class="update-dialog-foot">
          <button
            v-if="updater.status.value === 'available'"
            type="button"
            class="update-later"
            @click="updater.dismissDialog">
            稍后再说
          </button>
          <button
            type="button"
            class="update-install"
            :disabled="updater.status.value === 'downloading'"
            @click="updater.install">
            {{
              updater.status.value === "downloading"
                ? `下载中 ${updater.progressPercent.value}%`
                : "安装并重启"
            }}
          </button>
        </footer>
      </section>
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
  max-height: 100dvh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
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
.toast-stack {
  position: fixed;
  right: 18px;
  bottom: 74px;
  z-index: 50;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 8px;
  pointer-events: none;
}
.notice-toast {
  pointer-events: auto;
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
.notice-toast.warn {
  border-color: rgb(239 90 50 / 45%);
}
.notice-toast.warn svg {
  color: var(--signal);
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
.toast-enter-from {
  opacity: 0;
  transform: translateY(8px);
}

/* —— 更新信息弹窗 —— */
.update-dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgb(22 28 20 / 48%);
  backdrop-filter: blur(3px);
}
.update-dialog {
  width: min(480px, 100%);
  max-height: min(560px, calc(100dvh - 48px));
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 16px;
  background: var(--dock);
  color: var(--ink);
  box-shadow: 0 24px 64px rgb(22 28 20 / 35%);
}
.update-dialog-head {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 18px 18px 12px;
  border-bottom: 1px solid var(--line);
  background: var(--dock-0);
}
.update-dialog-icon {
  width: 40px;
  height: 40px;
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 10px;
  background: var(--acid);
  color: var(--ink);
}
.update-dialog-titles {
  min-width: 0;
  flex: 1;
}
.update-dialog-titles h2 {
  margin: 0;
  font: 700 17px/1.25 var(--font-display);
  letter-spacing: 0.01em;
}
.update-dialog-meta {
  margin: 4px 0 0;
  color: var(--muted);
  font: 550 12px/1.4 var(--font-mono);
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.update-dialog-close {
  width: 30px;
  height: 30px;
  flex: none;
  display: grid;
  place-items: center;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
}
.update-dialog-close:hover {
  background: rgb(31 39 28 / 8%);
  color: var(--ink);
}
.update-dialog-body {
  flex: 1;
  min-height: 120px;
  overflow: auto;
  padding: 14px 18px 8px;
}
.update-notes-empty {
  margin: 8px 0;
  color: var(--muted);
  font-size: 13.5px;
  line-height: 1.6;
}
.update-notes {
  border: 1px solid var(--line);
  border-radius: 12px;
  background: var(--surface);
  overflow: hidden;
}
.update-notes-label {
  padding: 8px 12px;
  border-bottom: 1px solid var(--line);
  background: var(--dock-0);
  color: var(--muted);
  font: 650 11px/1 var(--font-label);
  letter-spacing: 0.1em;
}
.update-notes-content {
  padding: 12px 14px 14px;
  font-size: 13.5px;
  line-height: 1.65;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--ink-2);
}
.update-progress {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 18px 4px;
}
.update-progress-track {
  flex: 1;
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: rgb(31 39 28 / 10%);
}
.update-progress-bar {
  height: 100%;
  border-radius: inherit;
  background: var(--acid-deep);
  transition: width 0.2s ease;
}
.update-progress-text {
  flex: none;
  color: var(--muted);
  font: 600 12px/1 var(--font-mono);
}
.update-dialog-foot {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 18px 16px;
  border-top: 1px solid var(--line);
  background: var(--dock-0);
}
.update-later {
  min-height: 38px;
  padding: 0 16px;
  border: 1px solid var(--line-strong);
  border-radius: 9px;
  background: transparent;
  color: var(--ink-2);
  cursor: pointer;
  font: 650 13px/1 var(--font-label);
}
.update-later:hover {
  background: rgb(31 39 28 / 5%);
}
.update-dialog .update-install {
  min-height: 38px;
  margin: 0;
  padding: 0 18px;
  border: 1px solid var(--ink);
  border-radius: 9px;
  background: var(--acid);
  color: var(--ink);
  cursor: pointer;
  font: 700 13.5px/1 var(--font-label);
  letter-spacing: 0.02em;
}
.update-dialog .update-install:disabled {
  opacity: 0.65;
  cursor: default;
}
.update-dialog .update-install:not(:disabled):hover {
  background: var(--acid-hi, #e6ff85);
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
  overflow: hidden;
}
.desktop-file-list {
  min-width: 0;
  min-height: 0;
  height: 100%;
  max-height: 100%;
  overflow: hidden;
}
.side-panel {
  min-width: 0;
  min-height: 0;
  height: 100%;
  max-height: 100%;
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

