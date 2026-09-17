<script setup lang="ts">
import { Check, Copy, ScanLine } from '@lucide/vue'
import { computed, shallowRef } from 'vue'
import QrcodeVue from 'qrcode.vue'

const props = defineProps<{
  accessUrl: string
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
  <aside class="share-card" aria-label="分享访问地址">
    <div class="card-index">ROUTE / 02</div>
    <div class="qr-frame">
      <QrcodeVue :value="accessUrl" :size="150" level="M" render-as="svg" />
      <span class="corner top-left" /><span class="corner top-right" />
      <span class="corner bottom-left" /><span class="corner bottom-right" />
    </div>
    <div class="scan-label"><ScanLine :size="15" /> 扫码让另一台设备接入</div>
    <button class="url-button" type="button" :title="accessUrl" @click="copyURL">
      <span>{{ shortURL }}</span>
      <Check v-if="copied" :size="16" aria-hidden="true" />
      <Copy v-else :size="16" aria-hidden="true" />
    </button>
  </aside>
</template>

<style scoped>
.share-card { position: relative; padding: 20px; border: 1px solid var(--line-strong); border-radius: 16px; background: var(--surface); box-shadow: 8px 8px 0 var(--ink); }
.card-index { margin-bottom: 18px; color: var(--muted); font: 650 9px/1 var(--font-label); letter-spacing: .18em; }
.qr-frame { padding: 19px; display: grid; place-items: center; position: relative; border-radius: 10px; background: #fff; box-shadow: inset 0 0 0 1px var(--line); }
.corner { width: 15px; height: 15px; position: absolute; border-color: var(--acid-deep); }
.top-left { top: 8px; left: 8px; border-top: 2px solid; border-left: 2px solid; }
.top-right { top: 8px; right: 8px; border-top: 2px solid; border-right: 2px solid; }
.bottom-left { bottom: 8px; left: 8px; border-bottom: 2px solid; border-left: 2px solid; }
.bottom-right { right: 8px; bottom: 8px; border-right: 2px solid; border-bottom: 2px solid; }
.scan-label { margin: 16px 0 12px; display: flex; align-items: center; gap: 7px; color: var(--muted); font-size: 11px; }
.url-button { width: 100%; min-width: 0; padding: 10px 0 0; display: flex; align-items: center; justify-content: space-between; gap: 10px; border: 0; border-top: 1px dashed var(--line-strong); color: var(--ink); background: transparent; cursor: pointer; font: 650 12px/1.2 var(--font-mono); }
.url-button span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.url-button:hover { color: var(--info); }
/* 手机上这张卡是被折叠起来的补充信息，而且用户正是扫码进来的——
   再给同一个地址看一遍二维码没有意义，只留可复制的地址 */
@media (max-width: 820px) {
  .share-card { width: auto; padding: 14px; box-shadow: 4px 4px 0 var(--ink); }
  .card-index { margin-bottom: 10px; }
  .qr-frame, .scan-label { display: none; }
  .url-button { padding-top: 0; border-top: 0; }
}
@media (max-width: 420px) {
  .share-card { box-shadow: 3px 3px 0 var(--ink); }
}
</style>
