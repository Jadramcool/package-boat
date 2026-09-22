<script setup lang="ts">
import { computed } from 'vue'
import { ArrowDownToLine, ArrowUpFromLine, Activity, Pause, Play, Square } from '@lucide/vue'
import SideCard from '@/components/SideCard.vue'
import type { ActiveTransfer, ConnectedDevice } from '@/types'
import { formatBytes } from '@/utils/format'

const props = defineProps<{
  transfers: readonly ActiveTransfer[]
  devices?: readonly ConnectedDevice[]
  running: boolean
}>()

const emit = defineEmits<{
  cancel: [id: string]
  pause: [id: string]
  resume: [id: string]
}>()

const summary = computed(() => {
  const down = props.transfers.filter(item => item.kind === 'download').length
  const up = props.transfers.filter(item => item.kind === 'upload').length
  const parts: string[] = []
  if (down > 0)
    parts.push(`${down} 个下载`)
  if (up > 0)
    parts.push(`${up} 个上传`)
  return parts.length > 0 ? parts.join(' · ') : '空闲'
})

/** 按 IP 匹配已连接设备，优先展示设备名，否则只显示 IP。 */
function peerLabel(item: ActiveTransfer): string {
  const ip = item.peer_ip
  if (!ip)
    return '未知设备'
  const match = props.devices?.find(device => device.ip === ip)
  if (match?.device_label)
    return match.device_label
  return ip
}

function speedLabel(bps: number): string {
  return `${formatBytes(bps)}/s`
}

function progressLabel(item: ActiveTransfer): string {
  if (item.progress != null)
    return `${item.progress}%`
  return formatBytes(item.done)
}

function rangeLabel(item: ActiveTransfer): string {
  if (item.total == null)
    return formatBytes(item.done)
  return `${formatBytes(item.done)} / ${formatBytes(item.total)}`
}

function onTogglePause(item: ActiveTransfer) {
  if (item.paused)
    emit('resume', item.id)
  else
    emit('pause', item.id)
}
</script>

<template>
  <SideCard title="传输中">
    <p v-if="!props.running" class="idle">
      局域网服务未运行。
    </p>
    <div v-else-if="props.transfers.length === 0" class="idle">
      当前没有进行中的上传或下载。设备开始传输后会实时出现在这里。
    </div>
    <template v-else>
      <div class="summary">
        <Activity :size="14" aria-hidden="true" />
        <span>{{ summary }}</span>
      </div>
      <ul class="transfer-list">
        <li v-for="item in props.transfers" :key="item.id" class="transfer-item" :class="{ paused: item.paused }">
          <!-- 第一行：标题 -->
          <div class="row-title">
            <span class="dir" :class="item.kind" aria-hidden="true">
              <ArrowDownToLine v-if="item.kind === 'download'" :size="12" />
              <ArrowUpFromLine v-else :size="12" />
            </span>
            <strong :title="item.name">{{ item.name }}</strong>
          </div>
          <!-- 第二行：状态 + 操作 -->
          <div class="row-status">
            <div class="status-meta">
              <span class="kind">{{ item.kind === 'download' ? '下载' : '上传' }}</span>
              <span class="state" :class="{ paused: item.paused }">
                {{ item.paused ? '已暂停' : '进行中' }}
              </span>
              <span class="pct">{{ progressLabel(item) }}</span>
              <span class="speed">{{ item.paused ? '—' : speedLabel(item.speed_bps) }}</span>
              <span class="peer" :title="item.peer_ip || '未知设备'">
                {{ peerLabel(item) }}
              </span>
              <span class="range">{{ rangeLabel(item) }}</span>
            </div>
            <div class="row-actions">
              <button
                type="button"
                class="action-button"
                :title="item.paused ? '继续' : '暂停'"
                :aria-label="item.paused ? '继续传输' : '暂停传输'"
                @click="onTogglePause(item)"
              >
                <Play v-if="item.paused" :size="12" :stroke-width="2.2" aria-hidden="true" />
                <Pause v-else :size="12" :stroke-width="2.2" aria-hidden="true" />
              </button>
              <button
                type="button"
                class="action-button stop"
                title="停止此传输"
                aria-label="停止此传输"
                @click="emit('cancel', item.id)"
              >
                <Square :size="11" :stroke-width="2.5" aria-hidden="true" />
              </button>
            </div>
          </div>
          <div
            class="bar"
            role="progressbar"
            :aria-valuenow="item.progress ?? undefined"
            aria-valuemin="0"
            aria-valuemax="100"
          >
            <span
              class="bar-fill"
              :class="[item.kind, { paused: item.paused }]"
              :style="{ width: `${item.progress ?? 8}%` }"
            />
          </div>
        </li>
      </ul>
    </template>
  </SideCard>
</template>

<style scoped>
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
.summary {
  display: flex;
  align-items: center;
  gap: 7px;
  color: var(--muted);
  font: 600 11.5px/1.2 var(--font-label);
}
.summary svg { flex: none; display: block; }
.transfer-list {
  margin: 0;
  padding: 0;
  display: grid;
  gap: 8px;
  list-style: none;
}
.transfer-item {
  padding: 9px 10px 10px;
  border: 1px solid var(--line);
  border-radius: 9px;
  background: var(--surface);
}
.transfer-item.paused {
  background: var(--dock-0);
}
/* 第一行：标题 */
.row-title {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  min-width: 0;
}
.row-title strong {
  min-width: 0;
  flex: 1;
  font: 680 12.5px/1.35 var(--font-display);
  overflow-wrap: anywhere;
  word-break: break-word;
  white-space: normal;
}
.dir {
  flex: none;
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  border-radius: 6px;
  margin-top: 1px;
}
.dir.download {
  color: var(--acid-deep);
  background: var(--acid-wash);
}
.dir.upload {
  color: #23559e;
  background: var(--info-wash);
}
.dir svg { display: block; }
/* 第二行：状态 + 操作 */
.row-status {
  margin-top: 7px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}
.status-meta {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px 8px;
  color: var(--muted);
  font: 550 10.5px/1.3 var(--font-mono);
}
.kind,
.state {
  flex: none;
  padding: 2px 6px;
  border-radius: 999px;
  font: 650 10px/1.3 var(--font-label);
}
.kind {
  color: var(--muted);
  background: var(--paper-deep, #e9ebe2);
}
.state {
  color: var(--acid-deep);
  background: var(--acid-wash);
}
.state.paused {
  color: var(--signal-deep);
  background: var(--signal-soft);
}
.pct {
  color: var(--ink);
  font: 700 11px/1.2 var(--font-mono);
  font-variant-numeric: tabular-nums;
}
.speed,
.range,
.peer {
  overflow-wrap: anywhere;
}
.peer {
  color: var(--ink);
  font-weight: 600;
}
.row-actions {
  flex: none;
  display: flex;
  align-items: center;
  gap: 5px;
}
.action-button {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border: 1px solid var(--line-strong);
  border-radius: 6px;
  background: var(--surface);
  color: var(--muted);
  cursor: pointer;
}
.action-button:hover {
  border-color: var(--ink);
  background: var(--ink);
  color: var(--paper);
}
.action-button.stop:hover {
  border-color: var(--signal);
  background: var(--signal);
  color: #fff;
}
.action-button svg { display: block; }
.bar {
  height: 5px;
  margin-top: 8px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--dock-0);
}
.bar-fill {
  display: block;
  height: 100%;
  border-radius: inherit;
  transition: width 200ms ease;
}
.bar-fill.download { background: var(--acid); }
.bar-fill.upload { background: #4d7fd6; }
.bar-fill.paused {
  opacity: 0.45;
  background: var(--muted);
}
</style>
