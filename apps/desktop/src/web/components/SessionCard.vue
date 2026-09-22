<script setup lang="ts">
import { Check, Copy } from '@lucide/vue'
import { computed, shallowRef } from 'vue'

const props = defineProps<{
  deviceName: string
  fileCount: number
  totalSizeLabel: string
  maxUploadLabel: string
  accessUrl?: string
}>()

const copied = shallowRef(false)
const shortURL = computed(() => (props.accessUrl ?? '').replace(/^https?:\/\//, ''))

async function copyURL(): Promise<void> {
  if (!props.accessUrl)
    return
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
  <section class="side-card" aria-labelledby="session-title">
    <header id="session-title">本机会话</header>
    <div class="body">
      <button
        v-if="accessUrl"
        class="addr-row mobile-only"
        type="button"
        :title="accessUrl"
        @click="copyURL"
      >
        <span class="addr-label">访问地址</span>
        <span class="addr-value">{{ shortURL }}</span>
        <Check v-if="copied" :size="14" aria-hidden="true" />
        <Copy v-else :size="14" aria-hidden="true" />
      </button>
      <dl class="stat-list">
        <div><dt>主机</dt><dd :title="deviceName">{{ deviceName }}</dd></div>
        <div><dt>货架</dt><dd>{{ fileCount }} 份</dd></div>
        <div><dt>占用</dt><dd>{{ totalSizeLabel }}</dd></div>
        <div><dt>单件上限</dt><dd>{{ maxUploadLabel }}</dd></div>
      </dl>
      <p class="note">文件只在当前局域网内流动，不经云端。</p>
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
.addr-row {
  width: 100%;
  margin: 0 0 12px;
  padding: 10px 12px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  grid-template-areas: 'label icon' 'value icon';
  gap: 3px 8px;
  align-items: center;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--dock-0);
  color: var(--ink);
  text-align: left;
  cursor: pointer;
}
.addr-label {
  grid-area: label;
  color: var(--muted);
  font: 600 10px/1 var(--font-label);
  letter-spacing: .08em;
  text-transform: uppercase;
}
.addr-value {
  grid-area: value;
  overflow: hidden;
  font: 600 12.5px/1.3 var(--font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.addr-row svg { grid-area: icon; }
.stat-list {
  margin: 0;
  display: grid;
  gap: 8px;
}
.stat-list > div {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}
.stat-list dt {
  flex: none;
  color: var(--muted);
  font: 600 11px/1.2 var(--font-label);
}
.stat-list dd {
  margin: 0;
  min-width: 0;
  overflow: hidden;
  color: var(--ink);
  font: 600 12.5px/1.3 var(--font-mono);
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.note {
  margin: 12px 0 0;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.5;
}
@media (max-width: 720px) {
  .mobile-only { display: grid; }
}
@media (min-width: 721px) {
  .mobile-only { display: none; }
}
</style>
