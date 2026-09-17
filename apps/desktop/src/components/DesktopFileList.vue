<script setup lang="ts">
import { computed } from 'vue'
import { FilePlus2, FolderUp, Inbox, LoaderCircle, RefreshCw, ShieldCheck, Trash2 } from '@lucide/vue'
import DesktopFileRow from '@/components/DesktopFileRow.vue'
import type { DesktopItem } from '@/types'
import { formatBytes } from '@/utils/format'

const props = defineProps<{
  items: readonly DesktopItem[]
  busy: string
  dragActive: boolean
  /** 清单概览卡移除后，原位/接收的分类汇总并入副标题这一行。 */
  linkedCount: number
  receivedCount: number
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
    <!-- 拖放条压成一行：拖拽入口要「够大够明显」，但不需要 108px 的纵向排场 -->
    <div class="drop-zone">
      <span class="drop-icon" aria-hidden="true">
        <FolderUp :size="16" :stroke-width="1.7" />
      </span>
      <p class="drop-copy">把文件拖到这里 · 仅保存原始路径，不复制、不移动</p>
      <button type="button" :disabled="props.busy === 'files'" @click="emit('choose')">
        <LoaderCircle v-if="props.busy === 'files'" class="spin" :size="14" />
        <FilePlus2 v-else :size="14" />
        选择文件
      </button>
    </div>

    <header class="list-header">
      <div class="list-heading">
        <h2 id="desktop-list-title">共享文件</h2>
        <p>{{ props.items.length }} 个文件 · {{ totalSize }} · 原位共享 {{ props.linkedCount }} · 远程接收 {{ props.receivedCount }}</p>
      </div>
      <div class="list-tools">
        <!-- 安全承诺贴着清空按钮：用户准备清空前最需要看到的就是这一句；
             清单为空时按钮本就禁用，这句话也就没有意义了 -->
        <span v-if="props.items.length > 0" class="safety-note"><ShieldCheck :size="13" aria-hidden="true" />清空只移除清单，不删除原文件</span>
        <button
          class="clear-button"
          type="button"
          :disabled="props.items.length === 0 || Boolean(props.busy)"
          :title="props.items.length === 0 ? '共享清单已为空' : '清空共享清单（不删除磁盘文件）'"
          @click="emit('clear')"
        >
          <LoaderCircle v-if="props.busy === 'clear'" class="spin" :size="14" />
          <Trash2 v-else :size="14" />
          一键清空
        </button>
        <button type="button" title="刷新文件状态" aria-label="刷新文件状态" @click="emit('refresh')">
          <RefreshCw :size="16" />
        </button>
      </div>
    </header>

    <div v-if="props.items.length === 0" class="empty-state">
      <Inbox :size="34" :stroke-width="1.4" />
      <strong>共享清单还是空的</strong>
      <p>添加后，手机连上同一局域网即可下载。</p>
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
.desktop-file-list { min-width: 0; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 16px; background: var(--surface); box-shadow: 0 8px 24px rgb(31 39 28 / 5%); transition: border-color 160ms ease, box-shadow 160ms ease; }
.desktop-file-list.drag-active { border-color: var(--acid-deep); box-shadow: 0 12px 34px rgb(92 122 18 / 18%); }
/* 46px 单行拖放条（原 40px）：虚线框内缩 5px，图标 28px，与标题行同属
   「内容区的入口带」；再高就会开始侵占数据行，故只放一档。 */
.drop-zone { position: relative; min-height: 46px; display: grid; grid-template-columns: 28px minmax(0, 1fr) auto; align-items: center; gap: 13px; padding: 0 16px; border-bottom: 1px solid var(--line); background: var(--dock-0); transition: background 160ms ease; }
.drop-zone::after { position: absolute; inset: 5px; border: 1px dashed var(--line-strong); border-radius: 9px; content: ''; pointer-events: none; }
.desktop-file-list.drag-active .drop-zone { background: var(--acid-wash); }
.desktop-file-list.drag-active .drop-zone::after { border-color: var(--acid-deep); }
.drop-icon { position: relative; z-index: 1; width: 28px; height: 28px; display: grid; place-items: center; border: 1px solid var(--ink); border-radius: 7px; background: var(--acid); color: var(--ink); }
.drop-copy { position: relative; z-index: 1; min-width: 0; margin: 0; overflow: hidden; color: var(--muted); font-size: 13.5px; text-overflow: ellipsis; white-space: nowrap; }
.drop-zone button { position: relative; z-index: 1; height: 32px; display: inline-flex; align-items: center; gap: 7px; padding: 0 15px; border: 1px solid var(--ink); border-radius: 7px; background: var(--ink); color: var(--paper); cursor: pointer; font: 680 13.5px/1 var(--font-label); transition: background 150ms ease, border-color 150ms ease; }
.drop-zone button:hover:not(:disabled) { background: #343c31; border-color: #343c31; }
.drop-zone button:disabled { cursor: wait; opacity: .55; }
/* 56px：清单标题是内容区的「页标题」，19px 时比行内的 13px 文件名只重一点，
   层级不足以撑起整张卡。放到 22px 后与拖放条、列头拉开明确的三级层次。 */
.list-header { min-height: 56px; display: flex; align-items: center; justify-content: space-between; gap: 18px; padding: 8px 16px; border-bottom: 1px solid var(--line); }
.list-heading h2 { margin: 0; font: 720 22px/1.1 var(--font-display); letter-spacing: -.02em; }
.list-heading p { margin: 5px 0 0; color: var(--muted); font-size: 13px; }
.list-tools { display: flex; align-items: center; gap: 11px; }
.list-tools button { width: 32px; height: 32px; display: grid; place-items: center; border: 1px solid var(--line-strong); border-radius: 7px; background: #fff; color: var(--ink); cursor: pointer; }
.list-tools button:hover { border-color: var(--ink); background: var(--ink); color: var(--paper); }
/* 清空是低频破坏性操作，常态用中性描边与普通文字色——不能用危险色抢注意力，
   否则会与「移除共享不会删除原文件」的核心承诺相互矛盾（用户会以为要删盘上文件）。
   危险色只在 hover 悬停、即用户真的准备点下去时才出现。 */
.list-tools .clear-button { width: auto; display: inline-flex; gap: 7px; padding: 0 11px; color: var(--muted); font: 650 12.5px/1 var(--font-label); }
.list-tools .clear-button:hover:not(:disabled) { border-color: var(--signal); background: var(--signal); color: #fff; }
.list-tools .clear-button:disabled { cursor: not-allowed; opacity: .45; }
/* 安全承诺：acid-wash 洗色底 + acid-deep 文字（已加深到 5.84:1，过 AA） */
.safety-note { display: inline-flex; align-items: center; gap: 6px; padding: 6px 10px; border-radius: 6px; background: var(--acid-wash); color: var(--acid-deep); font: 650 11.5px/1.3 var(--font-label); }
.safety-note svg { flex: none; }
.table-header { display: grid; grid-template-columns: minmax(260px, 1.5fr) minmax(82px, .42fr) 152px minmax(100px, .48fr) 68px; gap: 16px; padding: 11px 16px; border-bottom: 1px solid var(--line); color: var(--muted); font-size: 12.5px; }
.table-header span:last-child { text-align: right; }
.file-list { margin: 0; padding: 0; list-style: none; }
.empty-state { display: grid; place-content: center; justify-items: center; padding: 40px 24px; color: var(--muted); text-align: center; }
.empty-state strong { margin-top: 14px; color: var(--ink); font: 700 19px/1 var(--font-display); }
.empty-state p { margin: 10px 20px 0; font-size: 14px; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

@media (max-width: 1200px) {
  .table-header { grid-template-columns: minmax(250px, 1fr) 90px 100px 68px; gap: 16px; }
  .date-column { display: none; }
}
@media (max-width: 900px) {
  .list-header { padding-right: 14px; padding-left: 14px; }
  .list-tools { gap: 8px; }
  .safety-note { display: none; }
  .table-header { grid-template-columns: minmax(250px, 1fr) 90px 68px; gap: 12px; padding-right: 14px; padding-left: 14px; }
  .source-column { display: none; }
}
@media (max-width: 720px) {
  .drop-copy { font-size: 13px; }
  .list-header { flex-wrap: wrap; row-gap: 8px; }
  .list-heading h2 { font-size: 19px; }
}
@media (max-width: 640px) {
  .drop-zone { grid-template-columns: 28px minmax(0, 1fr); gap: 11px; padding: 9px 14px; }
  .drop-zone::after { inset: 5px; }
  .drop-zone button { grid-column: 2; justify-self: start; }
  .drop-copy { white-space: normal; line-height: 1.45; }
  .table-header { display: none; }
}
@media (max-width: 560px) {
  .list-header { min-height: 48px; padding: 6px 13px; }
  .list-heading h2 { font-size: 17px; }
  .list-heading p { font-size: 12px; }
  .list-tools button { width: 30px; height: 30px; }
  .empty-state strong { font-size: 17px; }
  .empty-state p { font-size: 13px; }
}
</style>
