<script setup lang="ts">
import { computed } from 'vue'
import { Eye, File, Folder, LoaderCircle, Trash2, Unlink } from '@lucide/vue'
import type { DesktopItem } from '@/types'
import { fileExtension, formatBytes, formatDate } from '@/utils/format'

const props = defineProps<{ item: DesktopItem; busy: string }>()
const emit = defineEmits<{ reveal: [id: string]; unshare: [id: string] }>()

const isBusy = computed(() => props.busy.endsWith(`:${props.item.id}`))
const isLinked = computed(() => props.item.source_type === 'linked')
const actionTitle = computed(() => isLinked.value ? '取消共享（不会删除原文件）' : '删除（从共享清单移除）')
</script>

<template>
  <li class="desktop-file-row" :class="{ missing: !props.item.available }">
    <div class="file-name-cell">
      <div class="file-stamp" :class="{ 'is-dir': props.item.is_dir }" aria-hidden="true">
        <Folder v-if="props.item.is_dir" :size="22" />
        <File v-else :size="22" />
        <span>{{ props.item.is_dir ? '文件夹' : fileExtension(props.item.name) }}</span>
      </div>
      <div class="file-main">
        <div class="name-line">
          <strong :title="props.item.name">{{ props.item.name }}</strong>
          <span v-if="!props.item.available" class="missing-badge">原文件已移动</span>
        </div>
        <p :title="props.item.local_path">{{ props.item.local_path }}</p>
        <div class="row-meta">
          <span>{{ props.item.is_dir ? '文件夹' : formatBytes(props.item.size) }}</span>
          <span>{{ formatDate(props.item.modified_at) }}</span>
          <span class="source-badge" :class="props.item.source_type">{{ isLinked ? '原位共享' : '远程接收' }}</span>
        </div>
      </div>
    </div>

    <div class="file-size">{{ props.item.is_dir ? '—' : formatBytes(props.item.size) }}</div>
    <div class="file-date">{{ formatDate(props.item.modified_at) }}</div>
    <div class="source-cell">
      <span class="source-badge" :class="props.item.source_type">
        {{ isLinked ? '原位共享' : '远程接收' }}
      </span>
    </div>
    <div class="row-actions">
      <button type="button" :disabled="!props.item.available || isBusy" title="在文件夹中查看" @click="emit('reveal', props.item.id)">
        <Eye :size="17" />
      </button>
      <button class="unshare" type="button" :disabled="isBusy" :title="actionTitle" @click="emit('unshare', props.item.id)">
        <LoaderCircle v-if="isBusy" class="spin" :size="17" />
        <Trash2 v-else-if="!isLinked" :size="17" />
        <Unlink v-else :size="17" />
      </button>
    </div>
  </li>
</template>

<style scoped>
.desktop-file-row { display: grid; grid-template-columns: minmax(260px, 1.5fr) minmax(82px, .42fr) minmax(118px, .55fr) minmax(100px, .48fr) 92px; align-items: center; gap: 18px; min-height: 76px; padding: 11px 20px; border-bottom: 1px solid var(--line); background: var(--surface); transition: background 120ms ease; }
.desktop-file-row:last-child { border-bottom: 0; }
.desktop-file-row:hover { background: #f4f2e9; }
.desktop-file-row.missing { opacity: .7; }
.file-name-cell { display: flex; align-items: center; gap: 14px; min-width: 0; }
.file-stamp { width: 48px; height: 54px; display: grid; grid-template-rows: 1fr auto; place-items: center; flex: none; padding: 8px 4px 6px; border: 1px solid var(--line-strong); border-radius: 5px; background: #fff; }
.file-stamp.is-dir { border-color: #8eaa25; background: #f7fadf; color: #5c6b15; }
.file-stamp span { font: 700 8px/1 var(--font-label); letter-spacing: .06em; }
.file-main { min-width: 0; }
.name-line { display: flex; align-items: center; gap: 8px; min-width: 0; }
.name-line strong { overflow: hidden; font: 680 16px/1.25 var(--font-display); text-overflow: ellipsis; white-space: nowrap; }
.source-badge, .missing-badge { flex: none; display: inline-flex; align-items: center; padding: 5px 8px; border-radius: 4px; font: 650 12px/1 var(--font-label); }
.source-badge.linked { background: #e7f7aa; color: #39440f; }
.source-badge.received { background: #e8e7e0; color: var(--ink); }
.missing-badge { background: var(--signal-soft); color: #7a260f; }
.file-main p { overflow: hidden; margin: 8px 0 0; color: var(--muted); font: 500 12px/1.2 var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
.row-meta { display: none; align-items: center; flex-wrap: wrap; gap: 8px 14px; margin-top: 8px; color: var(--muted); font: 550 12px/1.3 var(--font-mono); }
.row-meta .source-badge { font: 650 11px/1 var(--font-label); }
.file-size, .file-date { color: var(--muted); font: 550 14px/1.35 var(--font-mono); }
.row-actions { display: flex; justify-content: flex-end; gap: 8px; }
.row-actions button { width: 38px; height: 38px; display: grid; place-items: center; border: 1px solid var(--line-strong); border-radius: 5px; background: #fff; color: var(--ink); cursor: pointer; }
.row-actions button:hover:not(:disabled) { border-color: var(--ink); background: var(--ink); color: var(--paper); }
.row-actions .unshare:hover:not(:disabled) { border-color: var(--signal); background: var(--signal); }
.row-actions button:disabled { cursor: not-allowed; opacity: .4; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

@media (max-width: 1200px) {
  .desktop-file-row { grid-template-columns: minmax(250px, 1fr) 90px 100px 96px; gap: 16px; }
  .file-date { display: none; }
}
@media (max-width: 900px) {
  .desktop-file-row { grid-template-columns: minmax(250px, 1fr) 90px 96px; gap: 12px; padding: 11px 18px; }
  .source-cell { display: none; }
}
@media (max-width: 640px) {
  .desktop-file-row { grid-template-columns: minmax(0, 1fr) auto; gap: 12px; padding: 12px 18px; }
  .file-size, .file-date, .source-cell { display: none; }
  .file-main { display: flex; flex-direction: column; min-width: 0; }
  .row-meta { display: flex; order: 2; }
  .file-main p { order: 3; margin-top: 6px; }
}
@media (max-width: 560px) {
  .desktop-file-row { gap: 10px; padding: 8px 16px; }
  .file-name-cell { gap: 10px; }
  .file-stamp { width: 42px; height: 48px; }
  .name-line strong { font-size: 14px; }
  .file-main p { font-size: 11px; }
  .row-meta { gap: 6px 12px; font-size: 11px; }
  .source-badge { padding: 4px 7px; font-size: 11px; }
}
</style>
