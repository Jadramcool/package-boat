<script setup lang="ts">
import { computed } from 'vue'
import { ArrowDownToLine, ArrowUpFromLine, History, X } from '@lucide/vue'
import SideCard from '@/components/SideCard.vue'
import type { DesktopDevice, DesktopHistoryEntry } from '@/types'
import { formatBytes, formatDate } from '@/utils/format'

const props = defineProps<{
  history: readonly DesktopHistoryEntry[]
  devices?: readonly DesktopDevice[]
  retentionDays: number
  busy?: boolean
}>()

const emit = defineEmits<{
  clear: []
}>()

const summary = computed(() => {
  if (props.retentionDays === 0)
    return '历史已关闭'
  return `保留 ${props.retentionDays} 天 · ${props.history.length} 条`
})

function statusLabel(entry: DesktopHistoryEntry): string {
  if (entry.status === 'complete')
    return '完成'
  if (entry.status === 'cancelled')
    return '取消'
  return '中断'
}

function peerLabel(entry: DesktopHistoryEntry): string {
  const ip = entry.peer_ip
  if (!ip)
    return '本机'
  const match = props.devices?.find(device => device.ip === ip)
  if (match?.device_label)
    return match.device_label
  return ip
}

function sizeLabel(entry: DesktopHistoryEntry): string {
  if (entry.status === 'complete' || entry.total_bytes == null)
    return formatBytes(entry.transferred_bytes)
  return `${formatBytes(entry.transferred_bytes)} / ${formatBytes(entry.total_bytes)}`
}
</script>

<template>
  <SideCard title="传输历史">
    <div class="summary-row">
      <History :size="13" aria-hidden="true" />
      <span>{{ summary }}</span>
      <button
        v-if="props.history.length > 0"
        type="button"
        class="clear-btn"
        :disabled="props.busy"
        @click="emit('clear')"
      >
        <X :size="11" aria-hidden="true" />
        清空
      </button>
    </div>

    <p v-if="props.retentionDays === 0" class="idle">
      传输历史已关闭。可在下方「接收设置」中调整保留天数。
    </p>
    <p v-else-if="props.history.length === 0" class="idle">
      暂无历史记录。上传/下载结束后会出现在这里。
    </p>
    <ul v-else class="history-list">
      <li v-for="entry in props.history" :key="entry.id" class="history-row">
        <div class="row-title">
          <span class="dir" :class="entry.kind" aria-hidden="true">
            <ArrowDownToLine v-if="entry.kind === 'download'" :size="11" />
            <ArrowUpFromLine v-else :size="11" />
          </span>
          <strong :title="entry.name">{{ entry.name }}</strong>
          <span class="status" :class="entry.status">{{ statusLabel(entry) }}</span>
        </div>
        <div class="row-meta">
          <span>{{ entry.kind === 'download' ? '下载' : '上传' }}</span>
          <span class="mono">{{ sizeLabel(entry) }}</span>
          <span class="mono">{{ formatDate(entry.finished_at) }}</span>
          <span class="peer" :title="entry.peer_ip || '本机'">{{ peerLabel(entry) }}</span>
        </div>
      </li>
    </ul>
  </SideCard>
</template>

<style scoped>
.summary-row {
  display: flex;
  align-items: center;
  gap: 7px;
  color: var(--muted);
  font: 600 11.5px/1.2 var(--font-label);
}
.summary-row > svg { flex: none; display: block; }
.summary-row > span { flex: 1; min-width: 0; }
.clear-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 24px;
  padding: 0 8px;
  border: 1px solid var(--line-strong);
  border-radius: 6px;
  background: var(--surface);
  color: var(--muted);
  cursor: pointer;
  font: 650 10.5px/1 var(--font-label);
}
.clear-btn:hover:not(:disabled) {
  border-color: var(--signal);
  color: var(--signal);
}
.clear-btn:disabled { opacity: 0.55; cursor: not-allowed; }
.clear-btn svg { display: block; }
.idle {
  margin: 0;
  padding: 10px 11px;
  border: 1px dashed var(--line-strong);
  border-radius: 8px;
  background: var(--dock-0);
  color: var(--muted);
  font-size: 11.5px;
  line-height: 1.55;
}
.history-list {
  margin: 0;
  padding: 0;
  display: grid;
  gap: 6px;
  list-style: none;
  max-height: 220px;
  overflow-y: auto;
}
.history-row {
  padding: 8px 9px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--surface);
}
.row-title {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}
.row-title strong {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--ink);
  font: 680 12px/1.3 var(--font-display);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dir {
  flex: none;
  width: 20px;
  height: 20px;
  display: grid;
  place-items: center;
  border-radius: 5px;
}
.dir.download { color: var(--acid-deep); background: var(--acid-wash); }
.dir.upload { color: #23559e; background: var(--info-wash); }
.dir svg { display: block; }
.status {
  flex: none;
  padding: 2px 6px;
  border-radius: 999px;
  font: 650 10px/1.3 var(--font-label);
}
.status.complete { color: var(--acid-deep); background: var(--acid-wash); }
.status.cancelled { color: var(--muted); background: var(--paper-deep, #e9ebe2); }
.status.incomplete { color: var(--signal-deep); background: var(--signal-soft); }
.row-meta {
  margin-top: 5px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 8px;
  color: var(--muted);
  font: 550 10px/1.3 var(--font-mono);
}
.row-meta .peer {
  color: var(--ink);
  font-weight: 600;
}
.mono { font-variant-numeric: tabular-nums; }
</style>
