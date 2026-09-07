<script setup lang="ts">
import { Check, ChevronDown, CircleStop, Copy, ExternalLink, LoaderCircle, Network, Power } from '@lucide/vue'
import type { DesktopHostState } from '@/types'

const props = defineProps<{
  deviceName: string
  host: DesktopHostState
  accessUrl: string
  accessUrlIndex: number
  busy: boolean
  copied: string
}>()

const emit = defineEmits<{
  toggle: []
  copy: [url: string]
  open: [url: string]
  selectAddress: [url: string]
}>()

function handleAddressChange(event: Event) {
  emit('selectAddress', (event.target as HTMLSelectElement).value)
}
</script>

<template>
  <header class="desktop-header">
    <div class="brand-block">
      <div class="brand-mark" aria-hidden="true">L</div>
      <div class="brand-copy">
        <p>LANE 局域网投递站</p>
        <h1>{{ props.deviceName }}</h1>
      </div>
    </div>

    <div class="connection-block">
      <div class="conn-row">
        <div class="host-status" :class="{ online: props.host.running }">
          <span class="status-dot" aria-hidden="true" />
          <strong>{{ props.host.running ? '服务运行中' : '服务已停止' }}</strong>
        </div>
        <div v-if="props.host.running" class="code-pill" :title="`配对码：${props.host.access_code}`">
          <span>访问码</span>
          <strong>{{ props.host.access_code }}</strong>
        </div>
        <div v-if="props.host.running && props.host.urls.length > 1" class="address-switch">
          <Network :size="14" aria-hidden="true" />
          <label for="access-address">访问地址</label>
          <div class="select-wrap">
            <select id="access-address" :value="props.accessUrl" aria-label="切换局域网访问地址" @change="handleAddressChange">
              <option v-for="(url, index) in props.host.urls" :key="url" :value="url">
                {{ index === 0 ? '主地址' : `备用地址 ${index}` }} · {{ url }}
              </option>
            </select>
            <span>{{ props.accessUrlIndex + 1 }}/{{ props.host.urls.length }}</span>
            <ChevronDown :size="14" aria-hidden="true" />
          </div>
        </div>
      </div>
      <div class="conn-row">
        <div v-if="props.host.running && props.accessUrl" class="primary-url">
          <button type="button" :class="{ copied: props.copied === props.accessUrl }" :title="props.copied === props.accessUrl ? '已复制地址' : '复制访问地址'" @click="emit('copy', props.accessUrl)">
            <Check v-if="props.copied === props.accessUrl" :size="16" />
            <Copy v-else :size="16" />
            <span>{{ props.accessUrl }}</span>
          </button>
          <button type="button" title="用默认浏览器打开" aria-label="用默认浏览器打开" @click="emit('open', props.accessUrl)">
            <ExternalLink :size="16" />
          </button>
        </div>
        <span v-if="!props.host.running" class="stopped-hint">启动服务后生成访问地址与配对码</span>
      </div>
    </div>

    <div class="header-actions">
      <button class="power-button" type="button" :disabled="props.busy" @click="emit('toggle')">
        <LoaderCircle v-if="props.busy" class="spin" :size="20" />
        <CircleStop v-else-if="props.host.running" :size="20" />
        <Power v-else :size="20" />
        {{ props.host.running ? '停止服务' : '启动服务' }}
      </button>
    </div>
  </header>
</template>

<style scoped>
.desktop-header {
  display: grid;
  grid-template-columns: minmax(240px, 1fr) minmax(0, 1.5fr) auto;
  min-height: 100px;
  overflow: hidden;
  border: 1px solid #30372d;
  border-radius: 12px;
  background: var(--ink);
  color: var(--paper);
  box-shadow: 0 14px 34px rgb(22 27 20 / 12%);
}

.brand-block { display: flex; align-items: center; gap: 18px; min-width: 0; padding: 18px 24px; }
.brand-mark { width: 58px; height: 58px; display: grid; place-items: center; flex: none; border: 1.5px solid var(--acid); color: var(--acid); font: 800 28px/1 var(--font-display); transform: rotate(-3deg); }
.brand-copy { min-width: 0; }
.brand-copy p { margin: 0 0 8px; color: var(--paper); font: 650 14px/1 var(--font-label); letter-spacing: .08em; }
.brand-copy h1 { overflow: hidden; margin: 0; font: 720 25px/1 var(--font-display); letter-spacing: -.02em; text-overflow: ellipsis; white-space: nowrap; }

.connection-block { min-width: 0; display: grid; align-content: center; gap: 11px; padding: 14px 24px; border-left: 1px solid rgb(255 255 255 / 11%); }
.conn-row { display: flex; align-items: center; gap: 14px; min-width: 0; }
.host-status { display: flex; align-items: center; gap: 11px; flex: none; }
.host-status strong { font: 720 20px/1 var(--font-display); }
.status-dot { width: 12px; height: 12px; flex: none; border-radius: 50%; background: #747a70; box-shadow: 0 0 0 5px rgb(255 255 255 / 5%); }
.host-status.online { color: var(--acid); }
.host-status.online .status-dot { background: var(--acid); box-shadow: 0 0 0 5px rgb(217 255 82 / 12%); }
.code-pill { flex: none; display: inline-flex; align-items: center; gap: 8px; padding: 6px 11px; border: 1px solid rgb(217 255 82 / 26%); border-radius: 5px; background: rgb(217 255 82 / 7%); }
.code-pill span { color: rgb(241 238 228 / 58%); font: 600 11px/1 var(--font-label); }
.code-pill strong { color: var(--acid); font: 700 16px/1 var(--font-mono); letter-spacing: .12em; white-space: nowrap; }
.primary-url { display: flex; align-items: stretch; min-width: 0; }
.primary-url button { min-height: 32px; display: inline-flex; align-items: center; gap: 8px; min-width: 0; padding: 6px 10px; border: 1px solid rgb(255 255 255 / 16%); background: transparent; color: var(--paper); cursor: pointer; }
.primary-url button:first-child { flex: 1 1 auto; font: 550 13px/1.4 var(--font-mono); text-align: left; }
.primary-url button:last-child { width: 34px; flex: none; justify-content: center; padding: 0; border-left: 0; }
.primary-url button:hover { border-color: var(--acid); color: var(--acid); }
.primary-url button.copied { border-color: var(--acid); color: var(--acid); }
.primary-url button svg { flex: none; }
.primary-url span { overflow-wrap: anywhere; white-space: normal; }
.address-switch { display: flex; align-items: center; gap: 7px; min-width: 0; margin-left: auto; color: rgb(241 238 228 / 54%); font-size: 12px; }
.address-switch > svg { flex: none; color: var(--acid); }
.address-switch label { flex: none; }
.select-wrap { position: relative; min-width: 0; flex: 1 1 auto; max-width: 55%; }
.select-wrap select { width: 100%; height: 28px; padding: 0 58px 0 9px; overflow: hidden; border: 1px solid rgb(255 255 255 / 15%); border-radius: 4px; appearance: none; background: rgb(255 255 255 / 4%); color: var(--paper); cursor: pointer; font: 550 11px/1 var(--font-mono); text-overflow: ellipsis; white-space: nowrap; }
.select-wrap select:hover, .select-wrap select:focus-visible { border-color: var(--acid); outline: none; }
.select-wrap select option { background: var(--ink); color: var(--paper); }
.select-wrap span { position: absolute; top: 50%; right: 25px; color: var(--acid); font: 650 11px/1 var(--font-mono); pointer-events: none; transform: translateY(-50%); }
.select-wrap svg { position: absolute; top: 50%; right: 7px; pointer-events: none; transform: translateY(-50%); }
.stopped-hint { color: rgb(241 238 228 / 45%); font-size: 13px; }

.header-actions { display: flex; min-width: 0; }
.power-button { flex: 1; min-width: 134px; display: flex; align-items: center; justify-content: center; gap: 10px; border: 0; background: var(--acid); color: var(--ink); cursor: pointer; font: 720 16px/1 var(--font-label); }
.power-button:hover:not(:disabled) { background: #e4ff7c; }
.power-button:disabled { cursor: wait; opacity: .6; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 1120px) {
  .desktop-header { grid-template-columns: minmax(200px, 1fr) minmax(0, 1.35fr) auto; }
  .brand-block, .connection-block { padding-right: 18px; padding-left: 18px; }
  .brand-mark { width: 52px; height: 52px; font-size: 25px; }
  .brand-copy p { font-size: 13px; }
  .brand-copy h1 { font-size: 22px; }
  .power-button { min-width: 124px; font-size: 15px; }
}
@media (max-width: 900px) {
  .desktop-header { grid-template-columns: minmax(0, 1fr) auto; grid-template-rows: auto auto; }
  .brand-block { grid-column: 1; grid-row: 1; }
  .header-actions { grid-column: 2; grid-row: 1; border-left: 1px solid rgb(255 255 255 / 10%); }
  .connection-block { grid-column: 1 / -1; grid-row: 2; border-top: 1px solid rgb(255 255 255 / 10%); border-left: 0; padding: 12px 18px; }
  .power-button { min-height: 54px; }
  .address-switch label { display: none; }
}
@media (max-width: 720px) {
  .brand-block { gap: 14px; padding: 14px 18px; }
  .brand-mark { width: 46px; height: 46px; font-size: 22px; }
  .brand-copy p { margin-bottom: 6px; font-size: 12px; }
  .brand-copy h1 { font-size: 19px; }
  .host-status strong { font-size: 17px; }
  .code-pill { padding: 5px 9px; }
  .code-pill strong { font-size: 14px; }
  .connection-block { gap: 9px; }
  .power-button { min-width: 116px; min-height: 48px; font-size: 14px; }
}
@media (max-width: 560px) {
  .brand-block { padding: 12px 14px; }
  .brand-mark { width: 42px; height: 42px; font-size: 20px; }
  .brand-copy p { font-size: 11px; }
  .brand-copy h1 { font-size: 17px; }
  .connection-block { padding: 10px 14px; }
  .host-status strong { font-size: 15px; }
  .code-pill span { font-size: 10px; }
  .code-pill strong { font-size: 13px; }
  .primary-url button:first-child { font-size: 12px; }
  .power-button { min-width: 104px; min-height: 44px; font-size: 13px; }
}
</style>
