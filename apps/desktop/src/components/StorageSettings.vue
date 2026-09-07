<script setup lang="ts">
import { FolderCog, HardDrive, LoaderCircle, ShieldCheck } from '@lucide/vue'
import type { DesktopSettings } from '@/types'
import { formatBytes } from '@/utils/format'

const props = defineProps<{ settings: DesktopSettings; busy: boolean }>()
const emit = defineEmits<{ chooseDirectory: [] }>()
</script>

<template>
  <aside class="storage-settings" aria-labelledby="storage-title">
    <div class="settings-heading">
      <HardDrive :size="25" :stroke-width="1.5" />
      <div>
        <h2 id="storage-title">接收设置</h2>
        <p>其他设备上传的文件保存位置</p>
      </div>
    </div>

    <div class="path-section">
      <span class="path-label">保存位置</span>
      <div class="path-box" :title="props.settings.receive_dir">
        {{ props.settings.receive_dir }}
      </div>
    </div>
    <button type="button" :disabled="props.busy" @click="emit('chooseDirectory')">
      <LoaderCircle v-if="props.busy" class="spin" :size="18" />
      <FolderCog v-else :size="18" />
      更改目录
    </button>

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

    <div class="safety-note">
      <ShieldCheck :size="19" />
      <span>移除共享不会删除原文件</span>
    </div>
  </aside>
</template>

<style scoped>
.storage-settings { min-width: 0; padding: 22px; border: 1px solid var(--line-strong); border-radius: 12px; background: var(--surface); box-shadow: 0 8px 24px rgb(32 38 29 / 4%); }
.settings-heading { display: flex; align-items: flex-start; gap: 14px; }
.settings-heading svg { flex: none; margin-top: 2px; }
.settings-heading h2 { margin: 0; font: 720 25px/1.1 var(--font-display); letter-spacing: -.02em; }
.settings-heading p { margin: 7px 0 0; color: var(--muted); font-size: 14px; line-height: 1.5; }
.path-section { margin-top: 16px; }
.path-label { display: block; margin-bottom: 8px; color: var(--muted); font-size: 14px; }
.path-box { overflow: hidden; padding: 12px; border: 1px solid var(--line-strong); border-radius: 6px; background: #fff; font: 550 13px/1.5 var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
button { width: 100%; height: 44px; display: flex; align-items: center; justify-content: center; gap: 10px; margin-top: 10px; border: 1px solid var(--ink); border-radius: 6px; background: transparent; color: var(--ink); cursor: pointer; font: 680 15px/1 var(--font-label); }
button:hover:not(:disabled) { background: var(--ink); color: var(--paper); }
button:disabled { cursor: wait; opacity: .5; }
dl { margin: 14px 0 0; border-top: 1px solid var(--line); }
dl div { display: flex; justify-content: space-between; gap: 10px; padding: 11px 0; border-bottom: 1px solid var(--line); }
dt { color: var(--muted); font-size: 14px; }
dd { margin: 0; font: 680 14px/1 var(--font-mono); }
.safety-note { display: flex; align-items: center; gap: 10px; margin-top: 12px; padding: 12px; border-radius: 6px; background: #e7f7aa; color: #39440f; font-size: 14px; line-height: 1.5; }
.safety-note svg { flex: none; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 720px) {
  .storage-settings { padding: 16px; }
  .settings-heading { gap: 11px; }
  .settings-heading h2 { font-size: 20px; }
  .settings-heading p { font-size: 13px; }
  .path-section { margin-top: 14px; }
}
@media (max-width: 560px) {
  .storage-settings { padding: 14px; }
  .settings-heading h2 { font-size: 18px; }
  .settings-heading p { font-size: 12px; }
  .path-label { font-size: 12px; }
  .path-box { padding: 10px; font-size: 12px; }
  button { height: 40px; font-size: 13px; }
  dl { margin-top: 12px; }
  dt, dd { font-size: 12px; }
  .safety-note { margin-top: 10px; padding: 11px; font-size: 12px; }
}
</style>
