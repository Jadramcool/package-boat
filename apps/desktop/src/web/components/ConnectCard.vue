<script setup lang="ts">
import { Check, Copy, ScanLine } from '@lucide/vue'
import { computed, shallowRef } from 'vue'
import QrcodeVue from 'qrcode.vue'

const props = defineProps<{
  accessUrl: string
  deviceName: string
}>()

const copied = shallowRef(false)
const shortURL = computed(() => props.accessUrl.replace(/^https?:\/\//, ''))

async function copyURL(): Promise<void> {
  try {
    await navigator.clipboard.writeText(props.accessUrl)
  }
  catch {
    const textarea = document.createElement('textarea')
    textarea.value = props.accessUrl
    textarea.style.position = 'fixed'
    textarea.style.opacity = '0'
    document.body.append(textarea)
    textarea.select()
    document.execCommand('copy')
    textarea.remove()
  }
  copied.value = true
  window.setTimeout(() => {
    copied.value = false
  }, 1600)
}
</script>

<template>
  <section class="side-card" aria-labelledby="connect-title">
    <header id="connect-title">扫码接入</header>
    <div class="body">
      <div class="qr-frame">
        <QrcodeVue :value="accessUrl" :size="112" level="M" render-as="svg" />
      </div>
      <p class="qr-hint">手机相机扫码打开本页<br>（{{ deviceName }}）</p>
      <button class="addr-row" type="button" :title="accessUrl" @click="copyURL">
        <span class="addr-label">
          <ScanLine :size="12" aria-hidden="true" />
          <span>当前地址</span>
        </span>
        <span class="addr-value">{{ shortURL }}</span>
        <Check v-if="copied" :size="14" aria-hidden="true" />
        <Copy v-else :size="14" aria-hidden="true" />
      </button>
    </div>
  </section>
</template>

<style scoped>
.side-card {
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 16px;
  background: var(--surface);
  box-shadow: 0 6px 18px rgb(31 39 28 / 5%);
}
.side-card > header {
  padding: 10px 14px;
  border-bottom: 1px solid var(--line);
  background: var(--dock-0);
  font: 700 13px/1 var(--font-display);
}
.body { padding: 14px; }
.qr-frame {
  width: 136px;
  height: 136px;
  margin: 0 auto;
  display: grid;
  place-items: center;
  border: 1px solid var(--line-strong);
  border-radius: 12px;
  background: #fff;
}
.qr-hint {
  margin: 12px 0 0;
  text-align: center;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.55;
}
.addr-row {
  width: 100%;
  margin-top: 12px;
  padding: 10px 12px;
  display: grid;
  grid-template-columns: 1fr auto;
  grid-template-areas: 'label icon' 'value icon';
  gap: 4px 8px;
  align-items: center;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--dock-0);
  color: var(--ink);
  text-align: left;
  cursor: pointer;
}
.addr-row:hover { border-color: var(--line-strong); }
.addr-label {
  grid-area: label;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--muted);
  font: 600 9px/1.2 var(--font-label);
  letter-spacing: .1em;
}
.addr-label svg,
.addr-row > svg {
  flex: none;
  display: block;
}
.addr-value {
  grid-area: value;
  overflow: hidden;
  font: 600 12px/1.4 var(--font-mono);
  word-break: break-all;
}
.addr-row > svg { grid-area: icon; color: var(--muted); }
@media (max-width: 1099px) {
  .body {
    display: grid;
    grid-template-columns: 120px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .qr-frame { width: 120px; height: 120px; margin: 0; }
  .qr-hint { display: none; }
  .addr-row { margin-top: 0; }
}
@media (max-width: 560px) {
  .qr-frame { width: 100px; height: 100px; }
  .body { grid-template-columns: 100px minmax(0, 1fr); }
}
</style>
