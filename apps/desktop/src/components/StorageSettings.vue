<script setup lang="ts">
import { FolderCog, FolderOpen, LoaderCircle } from '@lucide/vue'
import SideCard from '@/components/SideCard.vue'
import type { DesktopSettings } from '@/types'
import { formatBytes } from '@/utils/format'

const props = defineProps<{ settings: DesktopSettings; busy: boolean }>()
const emit = defineEmits<{ chooseDirectory: [] }>()
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
      更改目录
    </button>

    <!-- 关闭窗口最小化到托盘的行为已在标题栏关闭按钮的 title 里说明，此处不再重复 -->

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
.path svg { flex: none; color: var(--muted); }
.path span { min-width: 0; overflow: hidden; color: var(--ink); font: 550 11.5px/1.4 var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
.choose { height: 34px; display: flex; align-items: center; justify-content: center; gap: 8px; border: 1px solid var(--line-strong); border-radius: 8px; background: #fff; color: var(--ink); cursor: pointer; font: 650 12.5px/1 var(--font-label); transition: background 140ms ease, border-color 140ms ease, color 140ms ease; }
.choose:hover:not(:disabled) { border-color: var(--ink); background: var(--ink); color: var(--paper); }
.choose:disabled { cursor: wait; opacity: .55; }
dl { display: grid; grid-template-columns: 1fr 1fr; gap: 1px; margin: 0; background: var(--line); }
dl div { padding: 9px 11px; background: var(--dock-0); }
dt { color: var(--muted); font: 600 10.5px/1 var(--font-label); letter-spacing: .04em; }
dd { margin: 5px 0 0; color: var(--ink); font: 700 13.5px/1 var(--font-mono); }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
