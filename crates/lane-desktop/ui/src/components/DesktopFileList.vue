<script setup lang="ts">
import { computed } from 'vue'
import { Download, FilePlus2, FolderUp, Inbox, Link2, LoaderCircle, RefreshCw, Trash2 } from '@lucide/vue'
import DesktopFileRow from '@/components/DesktopFileRow.vue'
import type { DesktopItem } from '@/types'
import { formatBytes } from '@/utils/format'

const props = defineProps<{
  items: readonly DesktopItem[]
  linkedCount: number
  receivedCount: number
  busy: string
  dragActive: boolean
}>()

const emit = defineEmits<{
  refresh: []
  reveal: [id: string]
  unshare: [id: string]
  choose: []
  clear: []
}>()

const totalSize = computed(() => formatBytes(props.items.reduce((total, item) => total + item.size, 0)))
</script>

<template>
  <section class="desktop-file-list" :class="{ 'drag-active': props.dragActive }" aria-labelledby="desktop-list-title">
    <div class="drop-zone">
      <div class="drop-icon" aria-hidden="true">
        <FolderUp :size="24" :stroke-width="1.35" />
      </div>
      <div class="drop-copy">
        <h3>把文件拖到这里</h3>
        <p>仅保存原始路径，不复制、不移动。文件始终保留在磁盘原处。</p>
      </div>
      <button type="button" :disabled="props.busy === 'files'" @click="emit('choose')">
        <LoaderCircle v-if="props.busy === 'files'" class="spin" :size="17" />
        <FilePlus2 v-else :size="17" />
        选择文件
      </button>
    </div>

    <header class="list-header">
      <div class="list-heading">
        <h2 id="desktop-list-title">共享文件</h2>
        <p>{{ props.items.length }} 个文件 · {{ totalSize }}</p>
      </div>
      <div class="catalog-stats">
        <span class="catalog-stat"><Link2 :size="13" aria-hidden="true" /><b>{{ props.linkedCount }}</b> 原位共享</span>
        <span class="catalog-stat"><Download :size="13" aria-hidden="true" /><b>{{ props.receivedCount }}</b> 远程接收</span>
        <button
          class="clear-button"
          type="button"
          :disabled="props.items.length === 0 || Boolean(props.busy)"
          title="清空共享清单（不删除磁盘文件）"
          @click="emit('clear')"
        >
          <LoaderCircle v-if="props.busy === 'clear'" class="spin" :size="15" />
          <Trash2 v-else :size="15" />
          一键清空
        </button>
        <button type="button" title="刷新文件状态" aria-label="刷新文件状态" @click="emit('refresh')">
          <RefreshCw :size="18" />
        </button>
      </div>
    </header>

    <div v-if="props.items.length === 0" class="empty-state">
      <Inbox :size="36" :stroke-width="1.4" />
      <strong>还没有共享文件</strong>
      <p>把文件拖到上方区域，或点击选择文件。</p>
    </div>
    <template v-else>
      <div class="table-header" aria-hidden="true">
        <span>文件名</span>
        <span class="size-column">大小</span>
        <span class="date-column">修改时间</span>
        <span class="source-column">来源</span>
        <span>操作</span>
      </div>
      <ul class="file-list">
        <DesktopFileRow
          v-for="item in props.items"
          :key="item.id"
          :item="item"
          :busy="props.busy"
          @reveal="emit('reveal', $event)"
          @unshare="emit('unshare', $event)"
        />
      </ul>
    </template>
  </section>
</template>

<style scoped>
.desktop-file-list { min-width: 0; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 12px; background: var(--surface); box-shadow: 0 8px 24px rgb(32 38 29 / 4%); transition: border-color 160ms ease, box-shadow 160ms ease; }
.desktop-file-list.drag-active { border-color: #8eaa25; box-shadow: 0 12px 34px rgb(122 146 31 / 16%); }
.drop-zone { position: relative; min-height: 108px; display: grid; grid-template-columns: 56px minmax(0, 1fr) auto; align-items: center; gap: 16px; padding: 18px 22px; overflow: hidden; border-bottom: 1px solid var(--line); background: var(--surface); transition: border-color 160ms ease, background 160ms ease; }
.drop-zone::after { position: absolute; inset: 14px; border: 1px dashed var(--line-strong); border-radius: 8px; content: ''; pointer-events: none; }
.desktop-file-list.drag-active .drop-zone { background: #f7fadf; border-bottom-color: #8eaa25; }
.desktop-file-list.drag-active .drop-zone::after { border-color: #8eaa25; }
.drop-icon { position: relative; z-index: 1; width: 56px; height: 56px; display: grid; place-items: center; border: 1px solid #8fa82b; border-radius: 8px; background: var(--acid); transition: transform 160ms ease; }
.desktop-file-list.drag-active .drop-icon { transform: scale(1.04); }
.drop-copy { position: relative; z-index: 1; min-width: 0; }
.drop-copy h3 { margin: 0 0 7px; font: 720 22px/1 var(--font-display); letter-spacing: -.02em; }
.drop-copy p { overflow: hidden; margin: 0; color: var(--muted); font-size: 13px; line-height: 1.5; text-overflow: ellipsis; white-space: nowrap; }
.drop-zone button { position: relative; z-index: 1; min-width: 128px; height: 42px; display: flex; align-items: center; justify-content: center; gap: 10px; padding: 0 20px; border: 1px solid var(--ink); border-radius: 7px; background: var(--ink); color: var(--paper); cursor: pointer; font: 680 15px/1 var(--font-label); transition: background 150ms ease, border-color 150ms ease, transform 150ms ease; }
.drop-zone button:hover:not(:disabled) { background: #343c31; border-color: #343c31; transform: translateY(-1px); }
.drop-zone button:disabled { cursor: wait; opacity: .55; }
.list-header { min-height: 74px; display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 16px 22px; border-bottom: 1px solid var(--line); }
.list-heading h2 { margin: 0; font: 720 27px/1 var(--font-display); letter-spacing: -.02em; }
.list-heading p { margin: 9px 0 0; color: var(--muted); font-size: 14px; }
.catalog-stats { display: flex; align-items: center; gap: 18px; }
.catalog-stat { display: inline-flex; align-items: center; gap: 6px; color: var(--muted); font-size: 13px; }
.catalog-stat svg { color: var(--line-strong); }
.catalog-stats b { color: var(--ink); font: 700 16px/1 var(--font-mono); }
.catalog-stats button { width: 40px; height: 40px; display: grid; place-items: center; border: 1px solid var(--line-strong); border-radius: 6px; background: #fff; color: var(--ink); cursor: pointer; }
.catalog-stats button:hover { border-color: var(--ink); background: var(--ink); color: var(--paper); }
.catalog-stats .clear-button { width: auto; min-height: 40px; display: inline-flex; align-items: center; gap: 7px; padding: 0 11px; border-color: #d7a99a; color: #8a321d; font: 650 12px/1 var(--font-label); }
.catalog-stats .clear-button:hover:not(:disabled) { border-color: var(--signal); background: var(--signal); color: #fff; }
.catalog-stats .clear-button:disabled { cursor: not-allowed; opacity: .42; }
.table-header { display: grid; grid-template-columns: minmax(260px, 1.5fr) minmax(82px, .42fr) minmax(118px, .55fr) minmax(100px, .48fr) 92px; gap: 18px; padding: 14px 20px; border-bottom: 1px solid var(--line); color: var(--muted); font-size: 13px; }
.table-header span:last-child { text-align: right; }
.file-list { margin: 0; padding: 0; list-style: none; }
.empty-state { display: grid; place-content: center; justify-items: center; padding: 44px 20px; color: var(--muted); text-align: center; }
.empty-state strong { margin-top: 16px; color: var(--ink); font: 700 20px/1 var(--font-display); }
.empty-state p { margin: 11px 20px 0; font-size: 15px; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

@media (max-width: 1200px) {
  .table-header { grid-template-columns: minmax(250px, 1fr) 90px 100px 96px; gap: 16px; }
  .date-column { display: none; }
}
@media (max-width: 900px) {
  .drop-zone { grid-template-columns: 48px minmax(0, 1fr); gap: 14px; padding: 18px; }
  .drop-icon { width: 48px; height: 48px; }
  .drop-zone button { grid-column: 2; justify-self: start; }
  .list-header { padding-right: 18px; padding-left: 18px; }
  .catalog-stats { gap: 12px; }
  .table-header { grid-template-columns: minmax(250px, 1fr) 90px 96px; gap: 12px; padding-right: 18px; padding-left: 18px; }
  .source-column { display: none; }
}
@media (max-width: 720px) {
  .drop-zone { grid-template-columns: 44px minmax(0, 1fr); }
  .drop-icon { width: 44px; height: 44px; }
  .drop-copy h3 { font-size: 19px; }
  .drop-zone button { height: 40px; min-width: 116px; padding: 0 16px; font-size: 14px; }
  .list-header { flex-wrap: wrap; row-gap: 12px; }
  .catalog-stats { flex-wrap: wrap; }
  .list-heading h2 { font-size: 22px; }
  .list-heading p { font-size: 13px; }
  .catalog-stats .clear-button { min-height: 36px; }
}
@media (max-width: 640px) {
  .drop-zone { grid-template-columns: 1fr; justify-items: center; gap: 10px; padding: 22px 16px; text-align: center; }
  .drop-zone::after { inset: 12px; }
  .drop-icon { width: 44px; height: 44px; }
  .drop-copy h3 { font-size: 18px; }
  .drop-copy p { white-space: normal; line-height: 1.5; }
  .drop-zone button { grid-column: auto; justify-self: center; width: min(100%, 220px); min-width: 0; }
  .table-header { display: none; }
}
@media (max-width: 560px) {
  .drop-zone { gap: 12px; padding: 12px 14px; }
  .drop-icon { width: 40px; height: 40px; }
  .drop-copy h3 { font-size: 17px; }
  .drop-copy p { font-size: 12px; }
  .drop-zone button { height: 38px; min-width: 104px; padding: 0 14px; font-size: 13px; }
  .list-header { min-height: 68px; gap: 14px; padding: 13px 14px; }
  .list-heading h2 { font-size: 20px; }
  .list-heading p { font-size: 12px; }
  .catalog-stats { gap: 10px; }
  .catalog-stat { font-size: 12px; }
  .catalog-stats b { font-size: 14px; }
  .empty-state strong { font-size: 18px; }
  .empty-state p { font-size: 14px; }
}
</style>
