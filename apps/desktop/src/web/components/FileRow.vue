<script setup lang="ts">
import {
  Download,
  File,
  FileArchive,
  FileAudio,
  FileImage,
  FileText,
  FileVideo,
  FolderArchive,
  LoaderCircle,
  Trash2,
} from '@lucide/vue'
import { computed } from 'vue'
import { fileExtension, formatBytes, formatDate } from '@/utils/format'
import type { SharedFile } from '../types'

const props = defineProps<{
  file: SharedFile
  deleting: boolean
}>()

const emit = defineEmits<{
  delete: [id: string, name: string]
}>()

const extension = computed(() => props.file.is_dir ? 'ZIP' : fileExtension(props.file.name))
const icon = computed(() => {
  if (props.file.is_dir)
    return FolderArchive
  const suffix = props.file.name.split('.').pop()?.toLowerCase() ?? ''
  if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'heic', 'bmp'].includes(suffix))
    return FileImage
  if (['mp4', 'mov', 'avi', 'mkv', 'webm'].includes(suffix))
    return FileVideo
  if (['mp3', 'wav', 'flac', 'm4a', 'aac', 'ogg'].includes(suffix))
    return FileAudio
  if (['zip', 'rar', '7z', 'tar', 'gz'].includes(suffix))
    return FileArchive
  if (['txt', 'md', 'pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'csv'].includes(suffix))
    return FileText
  return File
})
const downloadURL = computed(() => `/api/files/${encodeURIComponent(props.file.id)}/download`)
</script>

<template>
  <li class="file-row">
    <div class="file-type" aria-hidden="true">
      <component :is="icon" :size="23" :stroke-width="1.5" />
      <span>{{ extension }}</span>
    </div>
    <div class="file-info">
      <div class="file-name-line">
        <strong :title="file.name">{{ file.name }}</strong>
        <span class="source-label" :class="file.source_type">
          {{ file.source_type === 'linked' ? '原位共享' : '远程接收' }}
        </span>
        <span v-if="!file.available" class="missing-label">原文件已移动</span>
      </div>
      <div class="file-meta"><span>{{ formatBytes(file.size) }}</span><span>{{ formatDate(file.modified) }}</span></div>
    </div>
    <div class="file-actions">
      <a v-if="file.available" class="download-button" :href="downloadURL" :download="file.name" title="下载文件" aria-label="下载文件">
        <Download :size="17" aria-hidden="true" /><span>下载</span>
      </a>
      <span v-else class="unavailable-button" aria-label="文件不可用">不可用</span>
      <button
        type="button"
        class="delete-button"
        :disabled="deleting"
        :title="file.source_type === 'linked' ? '取消共享，不删除原文件' : '删除接收的文件'"
        :aria-label="file.source_type === 'linked' ? '取消共享' : '删除文件'"
        @click="emit('delete', file.id, file.name)"
      >
        <LoaderCircle v-if="deleting" class="spin" :size="16" aria-hidden="true" />
        <Trash2 v-else :size="16" aria-hidden="true" />
      </button>
    </div>
  </li>
</template>

<style scoped>
.file-row { min-height: 86px; padding: 14px 20px; display: grid; grid-template-columns: 58px minmax(0, 1fr) auto; align-items: center; gap: 17px; border-bottom: 1px solid var(--line); transition: background .15s; }
.file-row:last-child { border-bottom: 0; }
.file-row:hover { background: rgb(215 249 84 / 12%); }
.file-type { width: 58px; height: 58px; padding: 8px 4px 5px; display: grid; grid-template-rows: 1fr auto; place-items: center; border: 1px solid var(--line-strong); border-radius: 10px; color: var(--ink); background: var(--surface); }
.file-type span { font: 700 7px/1 var(--font-label); letter-spacing: .08em; }
.file-info { min-width: 0; }
.file-name-line { min-width: 0; display: flex; align-items: center; gap: 7px; }
.file-info strong { overflow: hidden; display: block; color: var(--ink); font: 650 14px/1.35 var(--font-display); text-overflow: ellipsis; white-space: nowrap; }
.source-label, .missing-label { flex: none; display: inline-flex; align-items: center; gap: 4px; padding: 4px 9px; border-radius: 999px; font: 650 10px/1 var(--font-label); letter-spacing: .04em; }
.source-label::before, .missing-label::before { content: ''; width: 5px; height: 5px; border-radius: 50%; background: currentColor; }
.source-label.linked { color: var(--acid-deep); background: var(--acid-wash); }
.source-label.received { color: #23559e; background: var(--info-wash); }
.missing-label { color: var(--signal-deep); background: var(--signal-soft); }
.file-meta { margin-top: 8px; display: flex; gap: 9px; color: var(--muted); font: 550 9px/1 var(--font-mono); }
.file-meta span + span::before { content: '·'; margin-right: 9px; }
.file-actions { display: flex; align-items: center; gap: 7px; }
.download-button, .unavailable-button, .delete-button { height: 36px; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--line-strong); border-radius: 9px; color: var(--ink); background: transparent; text-decoration: none; }
.download-button { padding: 0 12px; gap: 7px; font-size: 11px; }
.unavailable-button { padding: 0 10px; color: var(--muted); cursor: not-allowed; font-size: 9px; }
.delete-button { width: 36px; color: var(--muted); cursor: pointer; }
.download-button:hover { border-color: var(--ink); color: var(--paper); background: var(--ink); }
.delete-button:hover:not(:disabled) { border-color: var(--signal); color: var(--signal); }
.delete-button:disabled { opacity: .5; cursor: wait; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 540px) {
  .file-row { min-height: 74px; padding: 12px; grid-template-columns: 46px minmax(0, 1fr) auto; gap: 12px; }
  .file-type { width: 46px; height: 50px; }
  .file-info strong { font-size: 12px; }
  .file-meta { flex-direction: column; gap: 4px; }
  .file-meta span + span::before { display: none; }
  .download-button { width: 36px; padding: 0; }
  .download-button span { display: none; }
  .source-label { display: none; }
}
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
