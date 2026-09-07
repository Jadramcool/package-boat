<script setup lang="ts">
import { Files, LoaderCircle, RefreshCw } from '@lucide/vue'
import type { SharedFile } from '../types'
import FileRow from './FileRow.vue'

defineProps<{
  files: readonly SharedFile[]
  loading: boolean
  deletingId: string
}>()

const emit = defineEmits<{
  refresh: []
  delete: [id: string, name: string]
}>()
</script>

<template>
  <section class="file-shelf" aria-labelledby="shelf-title">
    <header class="shelf-header">
      <div><span class="section-index">SHELF / 05</span><h2 id="shelf-title">共享文件架</h2></div>
      <div class="shelf-tools">
        <span>{{ files.length }} ITEMS</span>
        <button type="button" :disabled="loading" title="刷新列表" aria-label="刷新列表" @click="emit('refresh')">
          <LoaderCircle v-if="loading" class="spin" :size="17" />
          <RefreshCw v-else :size="17" />
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
.file-shelf { margin-top: 60px; border: 1px solid var(--line-strong); background: var(--surface); }
.shelf-header { min-height: 88px; padding: 18px 20px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--line-strong); color: var(--paper); background: var(--ink); }
.section-index { color: var(--acid); font: 650 8px/1 var(--font-label); letter-spacing: .18em; }
.shelf-header h2 { margin: 8px 0 0; font: 700 22px/1 var(--font-display); }
.shelf-tools { display: flex; align-items: center; gap: 15px; color: rgb(244 241 232 / 50%); font: 600 9px/1 var(--font-label); letter-spacing: .13em; }
.shelf-tools button { width: 34px; height: 34px; display: grid; place-items: center; border: 1px solid rgb(255 255 255 / 20%); color: var(--paper); background: transparent; cursor: pointer; }
.shelf-tools button:hover:not(:disabled) { border-color: var(--acid); color: var(--acid); }
.shelf-tools button:disabled { opacity: .5; cursor: wait; }
.file-list { margin: 0; padding: 0; list-style: none; }
.shelf-state { min-height: 220px; display: grid; place-content: center; justify-items: center; color: var(--muted); text-align: center; }
.shelf-state h3 { margin: 14px 0 0; color: var(--ink); font: 700 19px/1 var(--font-display); }
.shelf-state p { max-width: 320px; margin: 8px 20px 0; font-size: 12px; line-height: 1.6; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 560px) {
  .file-shelf { margin-top: 38px; }
  .shelf-header { min-height: 76px; padding: 14px; }
}
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
