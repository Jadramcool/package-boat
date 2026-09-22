<script setup lang="ts">
import { computed } from 'vue'
import { MonitorSmartphone } from '@lucide/vue'
import SideCard from '@/components/SideCard.vue'
import type { ConnectedDevice } from '@/types'
import { formatDate } from '@/utils/format'

const props = defineProps<{
  devices: readonly ConnectedDevice[]
  running: boolean
}>()

const onlineCount = computed(() => props.devices.length)
const liveCount = computed(() => props.devices.filter(device => device.connected).length)

function modeLabel(mode: ConnectedDevice['auth_mode']): string {
  return mode === 'paired' ? '已配对' : '直连'
}

function relativeSeen(value: string): string {
  const time = Date.parse(value)
  if (Number.isNaN(time))
    return '刚刚'
  const seconds = Math.max(0, Math.round((Date.now() - time) / 1000))
  if (seconds < 15)
    return '刚刚'
  if (seconds < 60)
    return `${seconds} 秒前`
  if (seconds < 3600)
    return `${Math.floor(seconds / 60)} 分钟前`
  return formatDate(value)
}
</script>

<template>
  <SideCard title="已连接设备">
    <p v-if="!props.running" class="idle">
      局域网服务未运行，暂无设备活动记录。
    </p>
    <div v-else-if="props.devices.length === 0" class="idle">
      当前没有设备在线。手机扫码或输入配对码接入后会实时出现在这里，关闭页面后会自动移除。
    </div>
    <template v-else>
      <div class="summary">
        <MonitorSmartphone :size="14" aria-hidden="true" />
        <span>{{ onlineCount }} 台在线<template v-if="liveCount !== onlineCount"> · {{ liveCount }} 台保持连接</template></span>
      </div>
      <ul class="device-list">
        <li v-for="device in props.devices" :key="device.id" class="device-item">
          <div class="device-main">
            <div class="device-title">
              <strong :title="device.device_label">{{ device.device_label }}</strong>
              <span class="mode" :class="device.auth_mode">{{ modeLabel(device.auth_mode) }}</span>
              <span class="status online">
                {{ device.connected ? '连接中' : '在线' }}
              </span>
            </div>
            <p class="device-meta">
              <span>{{ device.ip }}</span>
              <span>最近 {{ relativeSeen(device.last_seen) }}</span>
            </p>
            <p v-if="device.user_agent" class="device-ua">{{ device.user_agent }}</p>
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
.device-list {
  margin: 0;
  padding: 0;
  display: grid;
  gap: 8px;
  list-style: none;
}
.device-item {
  padding: 9px 10px;
  border: 1px solid var(--line);
  border-radius: 9px;
  background: var(--surface);
}
.device-title {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}
.device-title strong {
  overflow: hidden;
  min-width: 0;
  font: 680 13px/1.2 var(--font-display);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mode,
.status {
  flex: none;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 7px;
  border-radius: 999px;
  font: 650 10px/1.3 var(--font-label);
}
.mode.paired {
  color: var(--acid-deep);
  background: var(--acid-wash);
}
.mode.anonymous {
  color: #23559e;
  background: var(--info-wash);
}
.status {
  margin-left: auto;
  color: var(--muted);
  background: var(--paper-deep, #e9ebe2);
}
.status.online {
  color: var(--acid-deep);
  background: var(--acid-wash);
}
.device-meta {
  margin: 6px 0 0;
  display: flex;
  flex-wrap: wrap;
  gap: 6px 12px;
  color: var(--muted);
  font: 550 11px/1.3 var(--font-mono);
}
.device-ua {
  margin: 5px 0 0;
  color: var(--muted);
  font: 500 10.5px/1.45 var(--font-mono);
  overflow-wrap: anywhere;
  word-break: break-word;
  white-space: normal;
}
</style>
