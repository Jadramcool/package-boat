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
.addr-row:hover { border-color: var(--line-strong); }
.addr-label {
  grid-area: label;
  color: var(--muted);
  font: 600 10px/1 var(--font-label);
  letter-spacing: .1em;
}
.addr-value {
  grid-area: value;
  overflow: hidden;
  font: 600 12px/1.35 var(--font-mono);
  word-break: break-all;
}
.addr-row > svg { grid-area: icon; color: var(--muted); }
.mobile-only { display: none; }
.stat-list { display: grid; gap: 10px; }
.stat-list div {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  align-items: baseline;
  font-size: 13px;
}
.stat-list dt { color: var(--muted); flex: none; }
.stat-list dd {
  min-width: 0;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
  font: 650 13px/1.2 var(--font-mono);
}
.note {
  margin-top: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--acid-wash);
  color: var(--acid-deep);
  font-size: 12px;
  line-height: 1.55;
}

/* 手机：置顶紧凑条 —— 地址独立描边 + 指标两列 */
@media (max-width: 1099px) {
  .side-card {
    border-radius: 12px;
    box-shadow: none;
  }
  .side-card > header {
    display: none;
  }
  .body {
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .mobile-only {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    margin: 0;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--dock-0);
    min-width: 0;
  }
  .addr-label {
    flex: none;
    font: 600 10px/1 var(--font-label);
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  .addr-value {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font: 600 12px/1.3 var(--font-mono);
    word-break: normal;
    text-align: left;
  }
  .mobile-only > svg {
    flex: none;
    display: block;
    color: var(--muted);
  }
  .stat-list {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 12px;
    margin: 0;
    padding: 0 2px 2px;
  }
  .stat-list div {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
    font-size: 11.5px;
    line-height: 1.3;
  }
  .stat-list dt {
    flex: none;
    color: var(--muted);
    font-size: 11px;
  }
  .stat-list dd {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font: 650 11.5px/1.2 var(--font-mono);
    text-align: right;
  }
  .note {
    display: none;
  }
}
</style>
