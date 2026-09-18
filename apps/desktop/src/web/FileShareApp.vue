<script setup lang="ts">
import { computed, onMounted } from "vue";
import { formatBytes } from "@/utils/format";
import BrandMark from "@/components/BrandMark.vue";
import AppHeader from "./components/AppHeader.vue";
import ConnectCard from "./components/ConnectCard.vue";
import FileShelf from "./components/FileShelf.vue";
import PairingGate from "./components/PairingGate.vue";
import SessionCard from "./components/SessionCard.vue";
import UploadDropzone from "./components/UploadDropzone.vue";
import UploadQueue from "./components/UploadQueue.vue";
import { useFileShare } from "./composables/useFileShare";

const share = useFileShare();
const accessURL = window.location.origin;
const deviceName = computed(
  () => share.serviceInfo.value?.device_name ?? "PacketBoat 文件站",
);
const totalSize = computed(() => formatBytes(share.totalBytes.value));
const maxUploadSize = computed(() =>
  formatBytes(share.serviceInfo.value?.max_upload_bytes ?? 0),
);
const linkedCount = computed(
  () =>
    share.files.value.filter((file) => file.source_type === "linked").length,
);
const receivedCount = computed(
  () =>
    share.files.value.filter((file) => file.source_type === "received").length,
);
const inboxCount = computed(
  () => share.files.value.filter((file) => file.source_type === "inbox").length,
);

onMounted(share.initialize);

async function confirmDelete(id: string, name: string): Promise<void> {
  const file = share.files.value.find((candidate) => candidate.id === id);
  if (file?.source_type === "linked") {
    if (window.confirm(`确定取消共享「${name}」吗？原文件不会被删除。`))
      await share.deleteFile(id);
    return;
  }
  const label = file?.source_type === "inbox" ? "本地文件" : "接收文件";
  if (
    window.confirm(
      `确定将${label}「${name}」移出共享清单吗？\n\n磁盘上的文件不会被删除。`,
    )
  )
    await share.deleteFile(id);
}
</script>

<template>
  <div v-if="share.initializing.value" class="boot-screen" aria-live="polite">
    <div class="boot-mark" aria-hidden="true"><BrandMark :size="40" /></div>
    <p class="boot-label">正在接入局域网投递站</p>
    <div class="boot-line" />
  </div>

  <PairingGate
    v-else-if="!share.authenticated.value"
    :server-name="deviceName"
    :loading="share.pairing.value"
    :offline="!share.online.value"
    :error="share.error.value"
    @submit="share.pair"
    @retry="share.initialize" />

  <div v-else class="app-shell">
    <AppHeader
      :device-name="deviceName"
      :online="share.online.value"
      :version="share.serviceInfo.value?.version ?? ''"
      @sign-out="share.signOut" />
    <div class="workspace">
      <h1 class="visually-hidden">PacketBoat 局域网文件接收页</h1>

      <div class="main-col">
        <p v-if="share.error.value" class="error-banner" role="alert">
          {{ share.error.value }}
        </p>
        <UploadDropzone
          :disabled="!share.online.value"
          :max-size-label="maxUploadSize"
          @select-files="share.addFiles" />
        <UploadQueue
          v-if="
            share.uploads.value.length > 0 || share.history.value.length > 0
          "
          :uploads="share.uploads.value"
          :history="share.history.value"
          :completed-count="share.completedUploads.value"
          :concurrency="share.uploadConcurrency.value"
          @cancel="share.cancelUpload"
          @pause="share.pauseUpload"
          @resume="share.resumeUpload"
          @retry="share.retryUpload"
          @clear="share.clearFinishedUploads"
          @clear-history="share.clearHistory"
          @update:concurrency="share.uploadConcurrency.value = $event" />
        <FileShelf
          :files="share.files.value"
          :loading="share.loadingFiles.value"
          :deleting-id="share.deletingID.value"
          :linked-count="linkedCount"
          :received-count="receivedCount"
          :inbox-count="inboxCount"
          :total-size-label="totalSize"
          @refresh="share.loadFiles"
          @delete="confirmDelete" />
      </div>

      <aside class="side-col" aria-label="连接与会话">
        <ConnectCard
          class="connect-card"
          :access-url="accessURL"
          :device-name="deviceName" />
        <SessionCard
          :device-name="deviceName"
          :file-count="share.files.value.length"
          :total-size-label="totalSize"
          :max-upload-label="maxUploadSize"
          :access-url="accessURL" />
      </aside>
    </div>
    <footer class="app-footer">
      <div class="footer-inner">
        <span>PacketBoat 接收页 · 局域网直达</span
        ><span>关闭主机程序即停止共享</span>
      </div>
    </footer>
  </div>
</template>

<style scoped>
.boot-screen {
  min-height: 100dvh;
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 18px;
  color: var(--paper);
  background: var(--ink);
}
.boot-mark {
  width: 68px;
  aspect-ratio: 1;
  display: grid;
  place-items: center;
  border: 2px solid var(--acid);
  border-radius: 18px;
  color: var(--acid);
}
.boot-label {
  margin: 0;
  font-size: 13px;
  letter-spacing: 0.14em;
}
.boot-line {
  width: 140px;
  height: 2px;
  overflow: hidden;
  background: rgb(255 255 255 / 14%);
}
.boot-line::after {
  content: "";
  width: 44%;
  height: 100%;
  display: block;
  background: var(--acid);
  animation: scan 1.1s ease-in-out infinite alternate;
}
.app-shell {
  min-height: 100dvh;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
}
.workspace {
  width: min(1240px, calc(100% - 48px));
  margin: 0 auto;
  padding: 18px 0 24px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  gap: 16px;
  align-items: start;
  align-content: start;
  align-self: stretch;
}
.main-col {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.side-col {
  position: sticky;
  top: 72px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-height: calc(100dvh - 92px);
  overflow-y: auto;
  overscroll-behavior: contain;
}
.error-banner {
  margin: 0;
  padding: 12px 14px;
  border: 1px solid var(--signal);
  border-radius: 10px;
  color: var(--signal-deep);
  background: var(--signal-soft);
  font-size: 13px;
}
.app-footer {
  width: 100%;
  border-top: 1px solid var(--line);
  background: var(--dock-0);
}
.footer-inner {
  width: min(1240px, calc(100% - 48px));
  margin: 0 auto;
  padding: 14px 0 18px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  color: var(--muted);
  font: 600 10.5px/1 var(--font-label);
  letter-spacing: 0.14em;
  text-transform: uppercase;
}
@keyframes scan {
  from {
    transform: translateX(-20%);
  }
  to {
    transform: translateX(150%);
  }
}
@media (max-width: 1099px) {
  .workspace {
    grid-template-columns: 1fr;
    width: min(calc(100% - 28px), 1240px);
    gap: 10px;
    padding: 12px 0 16px;
  }
  /* 手机：隐藏扫码卡；会话卡置顶成紧凑条，其后才是投递与文件架 */
  .connect-card {
    display: none;
  }
  .side-col {
    position: static;
    max-height: none;
    order: -1;
    gap: 0;
  }
  .main-col {
    gap: 10px;
  }
  .footer-inner {
    width: min(calc(100% - 28px), 1240px);
    align-items: flex-start;
    gap: 18px;
    line-height: 1.4;
  }
}
@media (prefers-reduced-motion: reduce) {
  .boot-line::after {
    animation: none;
  }
}
</style>

