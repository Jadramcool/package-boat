<script setup lang="ts">
import { FolderCog, FolderOpen, LoaderCircle, RefreshCw } from '@lucide/vue'
import SideCard from '@/components/SideCard.vue'
import type { DesktopSettings } from '@/types'
import { formatBytes } from '@/utils/format'

const props = defineProps<{
  settings: DesktopSettings
  busy: boolean
  rescanningInbox?: boolean
}>()
const emit = defineEmits<{
  chooseDirectory: []
  toggleShareReceiveDir: [enabled: boolean]
  rescanReceiveDir: []
}>()

function handleShareReceiveDirChange(event: Event) {
  emit('toggleShareReceiveDir', (event.target as HTMLInputElement).checked)
}
</script>

<template>
  <SideCard title="接收设置">
    <div class="field">
      <span class="field-label">接收目录</span>
      <div class="path" :title="props.settings.receive_dir">
        <FolderOpen :size="13" aria-hidden="true" />
        <span>{{ props.settings.receive_dir }}</span>
      </div>
    </div>

    <button type="button" class="choose" :disabled="props.busy" @click="emit('chooseDirectory')">
      <LoaderCircle v-if="props.busy" class="spin" :size="15" />
      <FolderCog v-else :size="15" />
      <span>更改目录</span>
    </button>

    <label class="switch">
      <span class="switch-copy">
        <strong>共享目录内全部文件</strong>
        <small>
          {{ props.settings.share_receive_dir
            ? '目录下顶层文件与文件夹会出现在共享清单'
            : '仅展示明确上传 / 原位共享的条目' }}
        </small>
      </span>
      <input
        type="checkbox"
        :checked="props.settings.share_receive_dir"
        aria-label="是否展示接收目录内全部文件"
        @change="handleShareReceiveDirChange"
      >
      <i aria-hidden="true" />
    </label>

    <!-- 独立刷新：移出清单的本地文件通过重新扫描加回，与开关键解耦 -->
    <button
      type="button"
      class="choose rescan"
      :disabled="!props.settings.share_receive_dir || props.busy || props.rescanningInbox"
      title="重新扫描接收目录，把已移出清单的本地文件加回来（不删除磁盘文件）"
      @click="emit('rescanReceiveDir')"
    >
      <LoaderCircle v-if="props.rescanningInbox" class="spin" :size="15" />
      <RefreshCw v-else :size="15" />
      <span>重新扫描接收目录</span>
    </button>
    <p v-if="props.settings.share_receive_dir" class="rescan-hint">
      移出清单的文件仍保留在磁盘上；点此扫描可重新展示。
    </p>

    <dl>
      <div>
        <dt>单文件上限</dt>
        <dd>{{ formatBytes(props.settings.max_upload_bytes) }}</dd>
      </div>
      <div>
        <dt>端口</dt>
        <dd>{{ props.settings.port }}</dd>
      </div>
    </dl>
  </SideCard>
</template>

<style scoped>
.field { min-width: 0; }
.field-label { display: block; margin-bottom: 6px; color: var(--muted); font: 600 11.5px/1 var(--font-label); }
.path { display: flex; align-items: center; gap: 7px; min-width: 0; padding: 8px 10px; border: 1px solid var(--line); border-radius: 8px; background: var(--dock-0); }
.path svg { flex: none; display: block; color: var(--muted); }
.path span { min-width: 0; overflow: hidden; color: var(--ink); font: 550 11.5px/1.4 var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
.choose { height: 34px; display: flex; align-items: center; justify-content: center; gap: 8px; border: 1px solid var(--line-strong); border-radius: 8px; background: #fff; color: var(--ink); cursor: pointer; font: 650 12.5px/1.2 var(--font-label); transition: background 140ms ease, border-color 140ms ease, color 140ms ease; }
.choose svg { flex: none; display: block; }
.choose:hover:not(:disabled) { border-color: var(--ink); background: var(--ink); color: var(--paper); }
.choose:disabled { cursor: not-allowed; opacity: .55; }
.rescan-hint { margin: 6px 0 0; color: var(--muted); font-size: 11px; line-height: 1.45; }
.switch { display: flex; align-items: center; justify-content: space-between; gap: 10px; cursor: pointer; }
.switch-copy { min-width: 0; }
.switch-copy strong { display: block; color: var(--ink); font: 650 12.5px/1.3 var(--font-label); }
.switch-copy small { display: block; margin-top: 4px; color: var(--muted); font-size: 11px; line-height: 1.5; }
.switch input { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
.switch i { position: relative; width: 36px; height: 21px; flex: none; border-radius: 999px; background: var(--line-strong); transition: background 160ms ease; }
.switch i::after { content: ''; position: absolute; top: 2px; left: 2px; width: 17px; height: 17px; border-radius: 50%; background: #fff; transition: left 160ms cubic-bezier(.22, 1, .36, 1); }
.switch input:checked + i { background: var(--info-lit); }
.switch input:checked + i::after { left: 17px; background: var(--ink); }
.switch input:focus-visible + i { outline: 3px solid color-mix(in srgb, var(--info-lit) 80%, transparent); outline-offset: 2px; }
dl { display: grid; grid-template-columns: 1fr 1fr; gap: 1px; margin: 0; background: var(--line); }
dl div { padding: 9px 11px; background: var(--dock-0); }
dt { color: var(--muted); font: 600 10.5px/1 var(--font-label); letter-spacing: .04em; }
dd { margin: 5px 0 0; color: var(--ink); font: 700 13.5px/1 var(--font-mono); }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>