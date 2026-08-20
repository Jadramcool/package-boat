<script setup lang="ts">
import { Check, Copy, ExternalLink } from '@lucide/vue'
import QrcodeVue from 'qrcode.vue'

const props = defineProps<{
  url: string
  qrUrl: string
  autoPairEnabled: boolean
  copied: boolean
}>()

const emit = defineEmits<{
  copy: [url: string]
  open: [url: string]
  toggleAutoPair: [enabled: boolean]
}>()

function handleAutoPairChange(event: Event) {
  emit('toggleAutoPair', (event.target as HTMLInputElement).checked)
}
</script>

<template>
  <aside class="access-qr-card" aria-labelledby="qr-title">
    <div class="qr-heading">
      <h2 id="qr-title">手机扫码访问</h2>
      <p>连接同一局域网后扫描</p>
    </div>

    <div class="qr-column">
      <div class="qr-frame" title="使用手机相机扫码访问">
        <div class="qr-code">
          <QrcodeVue :value="props.qrUrl" :size="122" level="M" render-as="svg" />
        </div>
      </div>
    </div>

    <div class="access-details">
      <button type="button" :title="props.copied ? '已复制地址' : '复制访问地址'" @click="emit('copy', props.url)">
        <Check v-if="props.copied" :size="16" />
        <Copy v-else :size="16" />
        <span>{{ props.copied ? '已复制' : props.url }}</span>
      </button>
      <label class="auto-pair-toggle">
        <span>
          <b>扫码自动带码</b>
          <small>{{ props.autoPairEnabled ? '扫码后直接进入' : '扫码后手动输入' }}</small>
        </span>
        <input
          type="checkbox"
          :checked="props.autoPairEnabled"
          aria-label="扫码时自动携带访问码"
          @change="handleAutoPairChange"
        >
        <i aria-hidden="true" />
      </label>
    </div>

    <button class="open-button" type="button" title="用默认浏览器打开" aria-label="用默认浏览器打开" @click="emit('open', props.url)">
      <ExternalLink :size="18" />
    </button>
  </aside>
</template>

<style scoped>
.access-qr-card {
  position: relative;
  min-height: 220px;
  display: grid;
  grid-template-columns: 138px minmax(0, 1fr);
  grid-template-rows: auto 1fr;
  gap: 14px 20px;
  padding: 22px;
  border: 1px solid var(--line-strong);
  border-radius: 12px;
  background: var(--surface);
  color: var(--ink);
  box-shadow: 0 8px 24px rgb(32 38 29 / 4%);
}

.qr-heading { grid-column: 1 / -1; }
.qr-heading h2 { margin: 0; font: 720 25px/1.1 var(--font-display); letter-spacing: -.02em; }
.qr-heading p { margin: 7px 0 0; color: var(--muted); font-size: 14px; }
.qr-column { display: grid; align-content: center; justify-items: center; }
.qr-frame { width: 138px; height: 138px; padding: 8px; border: 1px solid var(--line); border-radius: 8px; background: #fff; }
.qr-code { width: 122px; height: 122px; }
.qr-code :deep(svg) { display: block; width: 122px; height: 122px; }
.access-details { min-width: 0; align-self: center; }
.access-details button { width: 100%; height: 38px; display: flex; align-items: center; gap: 8px; overflow: hidden; padding: 0 11px; border: 1px solid var(--line-strong); border-radius: 6px; background: #fff; color: var(--ink); cursor: pointer; font: 550 12px/1 var(--font-mono); white-space: nowrap; }
.access-details button:hover { border-color: var(--ink); }
.access-details button svg { flex: none; }
.access-details button span { overflow: hidden; text-overflow: ellipsis; }
.auto-pair-toggle { display: flex; align-items: center; gap: 10px; margin-top: 11px; cursor: pointer; user-select: none; }
.auto-pair-toggle > span { min-width: 0; flex: 1; }
.auto-pair-toggle b { display: block; color: var(--ink); font: 650 12px/1.2 var(--font-label); }
.auto-pair-toggle small { display: block; margin-top: 4px; color: var(--muted); font-size: 10px; line-height: 1; }
.auto-pair-toggle input { position: absolute; width: 1px; height: 1px; overflow: hidden; opacity: 0; }
.auto-pair-toggle i { position: relative; width: 38px; height: 22px; flex: none; border: 1px solid var(--line-strong); border-radius: 999px; background: #d7d6cd; transition: background 150ms ease, border-color 150ms ease; }
.auto-pair-toggle i::after { content: ''; position: absolute; top: 3px; left: 3px; width: 14px; height: 14px; border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgb(0 0 0 / 18%); transition: transform 150ms ease; }
.auto-pair-toggle input:checked + i { border-color: #819e1d; background: var(--acid); }
.auto-pair-toggle input:checked + i::after { transform: translateX(16px); }
.auto-pair-toggle input:focus-visible + i { outline: 2px solid var(--ink); outline-offset: 2px; }
.open-button { position: absolute; top: 18px; right: 18px; width: 38px; height: 38px; display: grid; place-items: center; border: 1px solid var(--line-strong); border-radius: 6px; background: transparent; color: var(--ink); cursor: pointer; }
.open-button:hover { border-color: var(--ink); background: var(--ink); color: var(--paper); }

@media (max-width: 1250px) {
  .access-qr-card { grid-template-columns: 128px minmax(0, 1fr); gap: 14px; padding: 20px; }
  .qr-frame { width: 128px; height: 128px; padding: 7px; }
  .qr-code, .qr-code :deep(svg) { width: 112px; height: 112px; }
}

@media (max-width: 1100px) {
  .access-qr-card { grid-template-columns: 138px minmax(0, 1fr); }
}
@media (max-width: 900px) {
  .access-qr-card { min-height: 200px; grid-template-columns: 128px minmax(0, 1fr); padding: 20px; }
  .qr-frame { width: 128px; height: 128px; padding: 7px; }
  .qr-code, .qr-code :deep(svg) { width: 112px; height: 112px; }
}
@media (max-width: 720px) {
  .access-qr-card { gap: 12px 14px; padding: 16px; }
  .qr-heading h2 { font-size: 20px; }
  .qr-heading p { font-size: 13px; }
  .auto-pair-toggle { margin-top: 9px; }
}
@media (max-width: 560px) {
  .access-qr-card { grid-template-columns: 112px minmax(0, 1fr); gap: 10px 12px; padding: 14px; }
  .qr-frame { width: 112px; height: 112px; padding: 7px; }
  .qr-code, .qr-code :deep(svg) { width: 96px; height: 96px; }
  .qr-heading h2 { font-size: 18px; }
  .qr-heading p { font-size: 12px; }
  .access-details button { height: 36px; font-size: 11px; }
}
</style>
