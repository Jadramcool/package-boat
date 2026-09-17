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
const fileCount = computed(() => `${share.files.value.length} 份`)
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
    :server-name="share.serviceInfo.value?.device_name ?? 'PacketBoat 文件站'"
    :loading="share.pairing.value"
    :offline="!share.online.value"
    :error="share.error.value"
    @submit="share.pair"
    @retry="share.initialize"
  />

  <div v-else class="app-shell">
    <AppHeader
      :device-name="share.serviceInfo.value?.device_name ?? 'PacketBoat 文件站'"
      :online="share.online.value"
      :version="share.serviceInfo.value?.version ?? ''"
      @sign-out="share.signOut"
    />
    <main class="main-content">
      <!-- 打开折叠时文件架仍是这一页的主体，保留一个可被读屏识别的标题 -->
      <h1 class="visually-hidden">PacketBoat 局域网文件接收页</h1>

      <!-- 数据条：一瞥即知「这一页有多少东西」，取代原先占掉整个首屏的 hero -->
      <dl class="manifest-stats">
        <div><dt>当前货架</dt><dd>{{ fileCount }}</dd></div>
        <div><dt>占用空间</dt><dd>{{ totalSize }}</dd></div>
        <div><dt>单件上限</dt><dd>{{ maxUploadSize }}</dd></div>
      </dl>

      <!-- 品牌叙述与二维码都是「第一次看一遍就够」的内容，
           折起来把首屏让给真正要用的文件架 -->
      <details class="page-about">
        <summary>关于本页 · 扫码接入</summary>
        <div class="about-body">
          <p class="eyebrow"><span>LOCAL / 01</span> 局域网共享中</p>
          <p class="about-title">把文件放到<br><em>同一条网路上。</em></p>
          <p class="about-note">文件只在当前局域网内流动，未经云端。拖进来，另一台设备马上就能拿走。</p>
          <ShareCard :access-url="accessURL" />
        </div>
      </details>

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
      <span>PacketBoat 接收页 · 局域网直达</span><span>关闭主机程序即停止共享</span>
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
/* sticky 头自带间距，上边距只需 12px，不必再让出一整个 hero */
.main-content { width: min(1180px, calc(100% - 40px)); margin: 0 auto; padding: 12px 0 40px; }
/* 一行三格数据条：把原来铺满首屏的统计与标题压成一条横带 */
.manifest-stats { margin: 0 0 10px; display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 1px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 12px; background: var(--line); }
.manifest-stats div { min-width: 0; padding: 9px 12px; background: var(--surface); }
.manifest-stats dt { color: var(--muted); font: 600 10px/1.2 var(--font-label); letter-spacing: .1em; }
.manifest-stats dd { overflow: hidden; margin: 6px 0 0; color: var(--ink); font: 650 15px/1 var(--font-display); text-overflow: ellipsis; white-space: nowrap; }
/* 折叠面板：原生 details，键盘可达、读屏可播报展开状态 */
.page-about { margin: 0 0 12px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 12px; background: var(--surface); }
.page-about > summary { min-height: 36px; padding: 0 13px; display: flex; align-items: center; gap: 8px; color: var(--ink); cursor: pointer; font: 650 12.5px/1 var(--font-label); list-style: none; }
.page-about > summary::-webkit-details-marker { display: none; }
.page-about > summary::after { content: ''; width: 7px; height: 7px; margin-left: auto; flex: none; border-right: 1.6px solid var(--muted); border-bottom: 1.6px solid var(--muted); transform: rotate(45deg) translate(-2px, -2px); transition: transform .18s ease; }
.page-about[open] > summary::after { transform: rotate(-135deg) translate(-2px, -2px); }
.page-about > summary:hover { color: var(--info); }
.page-about > summary:focus-visible { outline: 2px solid var(--info); outline-offset: -2px; }
.about-body { padding: 14px 13px 16px; border-top: 1px solid var(--line); }
.eyebrow { margin: 0 0 12px; color: var(--muted); font: 650 11px/1 var(--font-label); letter-spacing: .17em; text-transform: uppercase; }
.eyebrow span { margin-right: 10px; padding: 4px 6px; color: var(--acid); background: var(--ink); }
.about-title { max-width: 760px; margin: 0; color: var(--ink); font: 720 clamp(26px, 7.4vw, 40px)/.98 var(--font-display); letter-spacing: -.05em; }
.about-title em { color: var(--signal); font-style: normal; }
.about-note { max-width: 590px; margin: 16px 0 0; color: var(--muted); font-size: 13.5px; line-height: 1.75; }
.error-banner { margin: 0 0 12px; padding: 12px 14px; border: 1px solid var(--signal); border-radius: 10px; color: var(--signal-deep); background: var(--signal-soft); font-size: 13px; }
.app-footer { width: min(1180px, calc(100% - 40px)); margin: 0 auto; padding: 20px 0 28px; display: flex; justify-content: space-between; border-top: 1px solid var(--line); color: var(--muted); font: 600 10px/1 var(--font-label); letter-spacing: .14em; text-transform: uppercase; }
@keyframes scan { from { transform: translateX(-20%); } to { transform: translateX(150%); } }
@media (max-width: 560px) {
  .main-content, .app-footer { width: min(calc(100% - 24px), 1180px); }
  .manifest-stats div { padding: 8px 10px; }
  .manifest-stats dt { font-size: 9.5px; letter-spacing: .06em; }
  .manifest-stats dd { font-size: 14px; }
  .about-body { padding: 12px 11px 14px; }
  .app-footer { align-items: flex-start; gap: 18px; line-height: 1.4; }
}
@media (prefers-reduced-motion: reduce) { .boot-line::after { animation: none; } }
</style>
