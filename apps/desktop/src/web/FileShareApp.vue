<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { formatBytes } from '@/utils/format'
import AppHeader from './components/AppHeader.vue'
import FileShelf from './components/FileShelf.vue'
import PairingGate from './components/PairingGate.vue'
import ShareCard from './components/ShareCard.vue'
import UploadDropzone from './components/UploadDropzone.vue'
import UploadQueue from './components/UploadQueue.vue'
import { useFileShare } from './composables/useFileShare'

const share = useFileShare()
const accessURL = window.location.origin
const fileCount = computed(() => `${share.files.value.length} 份文件`)
const totalSize = computed(() => formatBytes(share.totalBytes.value))
const maxUploadSize = computed(() => formatBytes(share.serviceInfo.value?.max_upload_bytes ?? 0))

onMounted(share.initialize)

async function confirmDelete(id: string, name: string): Promise<void> {
  const file = share.files.value.find(candidate => candidate.id === id)
  const message = file?.source_type === 'linked'
    ? `确定取消共享「${name}」吗？原文件不会被删除。`
    : `确定删除「${name}」吗？这会同时删除接收目录中的文件。`
  if (window.confirm(message))
    await share.deleteFile(id)
}
</script>

<template>
  <div v-if="share.initializing.value" class="boot-screen" aria-live="polite">
    <div class="boot-mark" aria-hidden="true"><span>L</span></div>
    <p class="boot-label">正在接入局域网投递站</p>
    <div class="boot-line" />
  </div>

  <PairingGate
    v-else-if="!share.authenticated.value"
    :server-name="share.serviceInfo.value?.device_name ?? 'LANE 文件站'"
    :loading="share.pairing.value"
    :offline="!share.online.value"
    :error="share.error.value"
    @submit="share.pair"
    @retry="share.initialize"
  />

  <div v-else class="app-shell">
    <AppHeader
      :device-name="share.serviceInfo.value?.device_name ?? 'LANE 文件站'"
      :online="share.online.value"
      :version="share.serviceInfo.value?.version ?? ''"
      @sign-out="share.signOut"
    />
    <main class="main-content">
      <section class="hero-grid" aria-labelledby="page-title">
        <div class="hero-copy">
          <p class="eyebrow"><span>LOCAL / 01</span> 局域网共享中</p>
          <h1 id="page-title">把文件放到<br><em>同一条网路上。</em></h1>
          <p class="hero-note">文件只在当前局域网内流动，未经云端。拖进来，另一台设备马上就能拿走。</p>
          <dl class="manifest-stats">
            <div><dt>当前货架</dt><dd>{{ fileCount }}</dd></div>
            <div><dt>占用空间</dt><dd>{{ totalSize }}</dd></div>
            <div><dt>单件上限</dt><dd>{{ maxUploadSize }}</dd></div>
          </dl>
        </div>
        <ShareCard :access-url="accessURL" />
      </section>

      <p v-if="share.error.value" class="error-banner" role="alert">{{ share.error.value }}</p>
      <UploadDropzone
        :disabled="!share.online.value"
        :max-size-label="maxUploadSize"
        @select-files="share.addFiles"
      />
      <UploadQueue
        v-if="share.uploads.value.length > 0"
        :uploads="share.uploads.value"
        :completed-count="share.completedUploads.value"
        @cancel="share.cancelUpload"
        @retry="share.retryUpload"
        @clear="share.clearFinishedUploads"
      />
      <FileShelf
        :files="share.files.value"
        :loading="share.loadingFiles.value"
        :deleting-id="share.deletingID.value"
        @refresh="share.loadFiles"
        @delete="confirmDelete"
      />
    </main>
    <footer class="app-footer">
      <span>LANE / LOCAL TRANSFER</span><span>关闭主机程序即停止共享</span>
    </footer>
  </div>
</template>

<style scoped>
.boot-screen { min-height: 100dvh; display: grid; place-content: center; justify-items: center; gap: 18px; color: var(--paper); background: var(--ink); }
.boot-mark { width: 68px; aspect-ratio: 1; display: grid; place-items: center; border: 2px solid var(--acid); border-radius: 50% 50% 8px; transform: rotate(45deg); }
.boot-mark span { transform: rotate(-45deg); font: 700 28px/1 var(--font-display); }
.boot-label { margin: 0; font-size: 13px; letter-spacing: .14em; }
.boot-line { width: 140px; height: 2px; overflow: hidden; background: rgb(255 255 255 / 14%); }
.boot-line::after { content: ''; width: 44%; height: 100%; display: block; background: var(--acid); animation: scan 1.1s ease-in-out infinite alternate; }
.app-shell { min-height: 100dvh; }
.main-content { width: min(1180px, calc(100% - 40px)); margin: 0 auto; padding: 74px 0 44px; }
.hero-grid { margin-bottom: 52px; display: grid; grid-template-columns: minmax(0, 1.55fr) minmax(300px, .7fr); align-items: end; gap: clamp(40px, 8vw, 112px); }
.eyebrow { margin: 0 0 20px; color: var(--muted); font: 650 12px/1 var(--font-label); letter-spacing: .17em; text-transform: uppercase; }
.eyebrow span { margin-right: 12px; padding: 5px 7px; color: var(--acid); background: var(--ink); }
.hero-copy h1 { max-width: 760px; margin: 0; color: var(--ink); font: 720 clamp(46px, 6.8vw, 88px)/.92 var(--font-display); letter-spacing: -.055em; }
.hero-copy h1 em { color: var(--signal); font-style: normal; }
.hero-note { max-width: 590px; margin: 28px 0 0; color: var(--muted); font-size: 16px; line-height: 1.8; }
.manifest-stats { margin: 42px 0 0; display: flex; flex-wrap: wrap; gap: 8px 30px; }
.manifest-stats div { min-width: 110px; padding-top: 12px; border-top: 1px solid var(--line-strong); }
.manifest-stats dt { color: var(--muted); font: 600 10px/1.2 var(--font-label); letter-spacing: .12em; }
.manifest-stats dd { margin: 7px 0 0; color: var(--ink); font: 650 15px/1 var(--font-display); }
.error-banner { margin: 0 0 18px; padding: 13px 16px; border: 1px solid color-mix(in srgb, var(--signal), black 10%); color: #5e1c0b; background: var(--signal-soft); font-size: 14px; }
.app-footer { width: min(1180px, calc(100% - 40px)); margin: 0 auto; padding: 26px 0 32px; display: flex; justify-content: space-between; border-top: 1px solid var(--line); color: var(--muted); font: 600 10px/1 var(--font-label); letter-spacing: .14em; text-transform: uppercase; }
@keyframes scan { from { transform: translateX(-20%); } to { transform: translateX(150%); } }
@media (max-width: 820px) {
  .main-content { padding-top: 48px; }
  .hero-grid { grid-template-columns: 1fr; gap: 34px; }
  .hero-copy h1 { font-size: clamp(44px, 13vw, 68px); }
}
@media (max-width: 560px) {
  .main-content, .app-footer { width: min(calc(100% - 24px), 1180px); }
  .main-content { padding-top: 34px; }
  .hero-grid { margin-bottom: 32px; }
  .hero-note { font-size: 14px; }
  .manifest-stats { gap: 20px; }
  .manifest-stats div { flex: 1; min-width: 90px; }
  .app-footer { align-items: flex-start; gap: 18px; line-height: 1.4; }
}
@media (prefers-reduced-motion: reduce) { .boot-line::after { animation: none; } }
</style>
