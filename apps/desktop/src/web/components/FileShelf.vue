<script setup lang="ts">
import { Files, LoaderCircle, RefreshCw } from '@lucide/vue'
import type { SharedFile } from '../types'
import FileRow from './FileRow.vue'

defineProps<{
  files: readonly SharedFile[]
  loading: boolean
  deletingId: string
  linkedCount: number
  receivedCount: number
  inboxCount?: number
  totalSizeLabel: string
}>()

const emit = defineEmits<{
  refresh: []
  delete: [id: string, name: string]
}>()
</script>

<template>
  <section class="file-shelf" aria-labelledby="shelf-title">
    <header class="shelf-header">
      <div class="shelf-title">
        <span class="section-index">SHELF</span>
        <div class="title-block">
          <h2 id="shelf-title">共享文件架</h2>
          <p class="title-sub">
            {{ files.length }} 件 · {{ totalSizeLabel }}
            · 原位 {{ linkedCount }} · 接收 {{ receivedCount }}<template v-if="inboxCount"> · 本地 {{ inboxCount }}</template>
          </p>
        </div>
      </div>
      <div class="shelf-tools">
        <button type="button" :disabled="loading" title="刷新列表" aria-label="刷新列表" @click="emit('refresh')">
          <LoaderCircle v-if="loading" class="spin" :size="15" />
          <RefreshCw v-else :size="15" />
        </button>
      </div>
    </header>
    <div v-if="loading && files.length === 0" class="shelf-state" aria-live="polite">
      <LoaderCircle class="spin" :size="25" /><p>正在清点文件</p>
    </div>
    <div v-else-if="files.length === 0" class="shelf-state empty">
      <Files :size="34" :stroke-width="1.4" aria-hidden="true" />
      <h3>货架还是空的</h3>
      <p>从上方投递第一份文件，它会立即出现在这里。</p>
    </div>
    <ul v-else class="file-list">
      <FileRow
        v-for="file in files"
        :key="file.id"
        :file="file"
        :deleting="deletingId === file.id"
        @delete="(id, name) => emit('delete', id, name)"
      />
    </ul>
  </section>
</template>

<style scoped>
.file-shelf {
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 14px;
  background: var(--surface);
  box-shadow: 0 8px 24px rgb(31 39 28 / 5%);
}
.shelf-header {
  min-height: 56px;
  padding: 10px 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border-bottom: 1px solid var(--line);
  color: var(--ink);
  background: var(--dock-0);
}
.shelf-title {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
}
.section-index {
  flex: none;
  padding-right: 12px;
  border-right: 1px solid var(--line-strong);
  color: var(--acid-deep);
  font: 650 10px/1 var(--font-label);
  letter-spacing: .16em;
}
.title-block { min-width: 0; }
.shelf-header h2 {
  margin: 0;
  overflow: hidden;
  font: 700 17px/1.15 var(--font-display);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.title-sub {
  margin: 4px 0 0;
  overflow: hidden;
  color: var(--muted);
  font: 600 11px/1.2 var(--font-label);
  letter-spacing: .04em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.shelf-tools {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
}
.shelf-tools button {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  border: 1px solid var(--line-strong);
  border-radius: 9px;
  color: var(--ink);
  background: var(--surface);
  cursor: pointer;
}
.shelf-tools button:hover:not(:disabled) {
  border-color: var(--ink);
  background: var(--ink);
  color: var(--paper);
}
.shelf-tools button:disabled {
  opacity: .5;
  cursor: wait;
}
.file-list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.shelf-state {
  min-height: 168px;
  display: grid;
  place-content: center;
  justify-items: center;
  color: var(--muted);
  text-align: center;
}
.shelf-state h3 {
  margin: 12px 0 0;
  color: var(--ink);
  font: 700 17px/1 var(--font-display);
}
.shelf-state p {
  max-width: 320px;
  margin: 7px 20px 0;
  font-size: 12px;
  line-height: 1.6;
}
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 560px) {
  .shelf-header {
    min-height: 44px;
    padding: 6px 12px;
  }
  .shelf-header h2 { font-size: 14px; }
  .title-sub { font-size: 9.5px; }
  .shelf-tools button {
    width: 28px;
    height: 28px;
  }
}
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
