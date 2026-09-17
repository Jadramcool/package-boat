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
        <Folder v-if="props.item.is_dir" :size="15" />
        <File v-else :size="15" />
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
          <span v-if="!props.item.available" class="source-badge missing">文件缺失</span>
          <span v-else class="source-badge" :class="props.item.source_type">{{ isLinked ? '原位共享' : '远程接收' }}</span>
        </div>
      </div>
    </div>

    <div class="file-size">{{ props.item.is_dir ? '—' : formatBytes(props.item.size) }}</div>
    <div class="file-date">{{ formatDate(props.item.modified_at) }}</div>
    <div class="source-cell">
      <span v-if="!props.item.available" class="source-badge missing">文件缺失</span>
      <span v-else class="source-badge" :class="props.item.source_type">
        {{ isLinked ? '原位共享' : '远程接收' }}
      </span>
    </div>
    <div class="row-actions">
      <button type="button" :disabled="!props.item.available || isBusy" title="在文件夹中查看" @click="emit('reveal', props.item.id)">
        <Eye :size="15" />
      </button>
      <button class="unshare" type="button" :disabled="isBusy" :title="actionTitle" @click="emit('unshare', props.item.id)">
        <LoaderCircle v-if="isBusy" class="spin" :size="15" />
        <Trash2 v-else-if="!isLinked" :size="15" />
        <Unlink v-else :size="15" />
      </button>
    </div>
  </li>
</template>

<style scoped>
/* 54px 行高：脚标 34px 是这一行最高的元素，内边距 8px 后上下各留 ~10px 余量，
   双行文件名（16.8 + 5 + 14.85）也刚好同高，两种内容都不会把行撑破。
   比 49px 松一档，但不参与头部的放大节奏——数据行是「量」的部分，
   单行再高会明显减少首屏行数。 */
/* 日期列固定 152px 而非 fr 份额：日期文案长度固定（`2025年8月31日 23:33`），
   13.5px 等宽下约 130px，给 22px 余量。用 .55fr 会被文件名列抢走份额而截断。 */
.desktop-file-row { display: grid; grid-template-columns: minmax(260px, 1.5fr) minmax(82px, .42fr) 152px minmax(100px, .48fr) 68px; align-items: center; gap: 16px; min-height: 54px; padding: 8px 16px; border-bottom: 1px solid var(--line); background: var(--surface); transition: background 120ms ease; }
.desktop-file-row:last-child { border-bottom: 0; }
.desktop-file-row:hover { background: rgb(215 249 84 / 12%); }
.desktop-file-row.missing { opacity: .72; }
.file-name-cell { display: flex; align-items: center; gap: 11px; min-width: 0; }
.file-stamp { width: 32px; height: 34px; display: grid; grid-template-rows: 1fr auto; place-items: center; flex: none; padding: 4px 2px 3px; border: 1px solid var(--line-strong); border-radius: 7px; background: #fff; }
.file-stamp.is-dir { border-color: color-mix(in srgb, var(--acid-deep) 45%, transparent); background: var(--acid-wash); color: var(--acid-deep); }
.file-stamp span { overflow: hidden; max-width: 100%; font: 700 7.5px/1 var(--font-label); text-overflow: ellipsis; white-space: nowrap; }
.file-main { min-width: 0; }
.name-line { display: flex; align-items: center; gap: 8px; min-width: 0; }
.name-line strong { overflow: hidden; font: 680 14px/1.2 var(--font-display); text-overflow: ellipsis; white-space: nowrap; }
.source-badge, .missing-badge { flex: none; display: inline-flex; align-items: center; gap: 4px; padding: 3px 8px; border-radius: 999px; font: 650 11px/1.4 var(--font-label); }
.source-badge::before { content: ''; width: 5px; height: 5px; flex: none; border-radius: 50%; background: currentColor; }
.source-badge.linked { background: var(--acid-wash); color: var(--acid-deep); }
.source-badge.received { background: var(--info-wash); color: #23559e; }
.source-badge.missing { background: var(--signal-soft); color: var(--signal-deep); }
.missing-badge { background: var(--signal-soft); color: var(--signal-deep); }
.file-main p { overflow: hidden; margin: 5px 0 0; color: var(--muted); font: 500 11.5px/1.25 var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
.row-meta { display: none; align-items: center; flex-wrap: wrap; gap: 6px 12px; margin-top: 8px; color: var(--muted); font: 550 12px/1.3 var(--font-mono); }
.row-meta .source-badge { font: 650 11px/1 var(--font-label); }
.file-size, .file-date { overflow: hidden; color: var(--muted); font: 550 13.5px/1.35 var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
.row-actions { display: flex; justify-content: flex-end; gap: 6px; }
.row-actions button { width: 30px; height: 30px; display: grid; place-items: center; border: 1px solid var(--line-strong); border-radius: 7px; background: #fff; color: var(--ink); cursor: pointer; }
.row-actions button:hover:not(:disabled) { border-color: var(--ink); background: var(--ink); color: var(--paper); }
.row-actions .unshare:hover:not(:disabled) { border-color: var(--signal); background: var(--signal); }
.row-actions button:disabled { cursor: not-allowed; opacity: .4; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

@media (max-width: 1200px) {
  .desktop-file-row { grid-template-columns: minmax(250px, 1fr) 90px 100px 68px; }
  .file-date { display: none; }
}
@media (max-width: 900px) {
  .desktop-file-row { grid-template-columns: minmax(250px, 1fr) 90px 68px; gap: 12px; padding: 8px 14px; }
  .source-cell { display: none; }
}
@media (max-width: 640px) {
  .desktop-file-row { grid-template-columns: minmax(0, 1fr) auto; gap: 10px; padding: 10px 13px; }
  .file-size, .file-date, .source-cell { display: none; }
  .file-main { display: flex; flex-direction: column; min-width: 0; }
  .row-meta { display: flex; order: 2; }
  .file-main p { order: 3; margin-top: 5px; }
}
@media (max-width: 560px) {
  .file-name-cell { gap: 9px; }
  .file-stamp { width: 30px; height: 32px; }
  .name-line strong { font-size: 13px; }
  .file-main p { font-size: 11px; }
  .row-meta { gap: 5px 10px; font-size: 11.5px; }
  .source-badge { padding: 3px 7px; font-size: 10.5px; }
}
</style>
