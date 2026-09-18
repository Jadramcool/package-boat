<script setup lang="ts">
import {
  AlertTriangle,
  Check,
  CircleStop,
  Copy,
  ExternalLink,
  LoaderCircle,
  Power,
} from '@lucide/vue'
import type { DesktopHostState, LocalAddress } from '@/types'

const props = defineProps<{
  deviceName: string
  host: DesktopHostState
  accessUrl: string
  activeAddress: LocalAddress | null
  addressUnreachable: boolean
  busy: boolean
  copied: string
}>()

const emit = defineEmits<{
  toggle: []
  copy: [url: string]
  open: [url: string]
}>()
</script>

<template>
  <!-- 命令条：把驾驶舱压成一行——常看的三件事（在跑吗 / 地址 / 配对码）留在面上，
       切换地址、扫码行为、接收目录等低频设置移到右侧边栏。 -->
  <header class="cmdbar" aria-label="服务命令条">
    <span class="brand" aria-hidden="true">L</span>

    <div class="identity">
      <div class="identity-text">
        <strong class="device" :title="props.deviceName">{{ props.deviceName }}</strong>
        <span class="product">局域网投递站</span>
      </div>
      <span class="state" :class="{ online: props.host.running }">
        <i aria-hidden="true" />
        {{ props.host.running ? '运行中' : '已停止' }}
      </span>
    </div>

    <template v-if="props.host.running && props.accessUrl">
      <span class="url" :class="{ warn: props.addressUnreachable }">
        <AlertTriangle v-if="props.addressUnreachable" :size="14" aria-hidden="true" />
        <code :title="props.addressUnreachable ? `${props.accessUrl}（手机可能无法直连，见右侧连接设置）` : props.accessUrl">
          {{ props.accessUrl }}
        </code>
        <button
          type="button"
          class="chip-action"
          :class="{ ok: props.copied === props.accessUrl }"
          :title="props.copied === props.accessUrl ? '已复制地址' : '复制访问地址'"
          @click="emit('copy', props.accessUrl)"
        >
          <Check v-if="props.copied === props.accessUrl" :size="13" />
          <Copy v-else :size="13" />
          {{ props.copied === props.accessUrl ? '已复制' : '复制' }}
        </button>
        <button
          type="button"
          class="chip-action icon-only"
          title="用默认浏览器打开"
          aria-label="用默认浏览器打开"
          @click="emit('open', props.accessUrl)"
        >
          <ExternalLink :size="13" />
        </button>
      </span>

      <span class="code" :title="`配对码：${props.host.access_code}`">
        <strong>{{ props.host.access_code }}</strong>
      </span>
    </template>

    <p v-else class="idle-hint">启动服务后这里会显示访问地址与配对码</p>

    <span class="spacer" />

    <button
      class="power"
      :class="{ 'is-running': props.host.running }"
      type="button"
      :disabled="props.busy"
      @click="emit('toggle')"
    >
      <LoaderCircle v-if="props.busy" class="spin" :size="16" />
      <CircleStop v-else-if="props.host.running" :size="16" />
      <Power v-else :size="16" />
      {{ props.host.running ? '停止服务' : '启动服务' }}
    </button>
  </header>
</template>

<style scoped>
/* 64px：56px 时 14px 设备名与 30px 地址框挤在一条 56px 的深色带里，
   两侧各留 16px 后视觉重心贴边。放宽到 64px 后 gap 14px、内边距 20px，
   控件与带边的关系恢复「有呼吸的容器」而不是「刚好塞下」。
   代价：首屏可见行数 9 → 8（这是明知的取舍）。 */
.cmdbar {
  display: flex;
  align-items: center;
  gap: 14px;
  height: 64px;
  padding: 0 20px;
  background: var(--ink);
  color: var(--paper);
  border-top: 1px solid rgb(251 252 247 / 10%);
}
.brand {
  width: 32px;
  height: 32px;
  flex: none;
  display: grid;
  place-items: center;
  border: 1.5px solid var(--acid);
  color: var(--acid);
  font: 800 15px/1 var(--font-display);
  transform: rotate(-3deg);
}
.identity {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
}
.identity-text {
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 3px;
}
.device {
  max-width: 170px;
  overflow: hidden;
  font: 700 15.5px/1 var(--font-display);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.product {
  color: rgb(251 252 247 / 48%);
  font: 600 10px/1 var(--font-label);
  letter-spacing: 0.12em;
  white-space: nowrap;
}
.state {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex: none;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgb(255 255 255 / 8%);
  color: rgb(251 252 247 / 72%);
  font: 650 11.5px/1 var(--font-label);
}
.state i {
  width: 7px;
  height: 7px;
  flex: none;
  border-radius: 50%;
  background: #79826f;
}
.state.online {
  background: rgb(120 182 255 / 16%);
  color: var(--info-lit);
}
.state.online i {
  background: var(--info-lit);
}

.url { flex: 1; min-width: 0; max-width: 340px; display: flex; align-items: center; gap: 7px; height: 36px; padding: 0 5px 0 12px; border: 1px solid rgb(255 255 255 / 16%); border-radius: 9px; background: rgb(255 255 255 / 5%); }
.url > svg { flex: none; color: #ffd6a8; }
.url > code { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 550 12.5px/1 var(--font-mono); }
.url.warn { border-color: rgb(255 195 120 / 55%); background: rgb(255 170 90 / 12%); }

.chip-action { flex: none; height: 26px; display: inline-flex; align-items: center; gap: 5px; padding: 0 10px; border: 0; border-radius: 6px; background: rgb(255 255 255 / 10%); color: var(--paper); cursor: pointer; font: 650 11px/1 var(--font-label); transition: background 140ms ease, color 140ms ease; }
.chip-action:hover { background: rgb(215 249 84 / 20%); color: var(--acid); }
.chip-action.ok { background: var(--acid); color: var(--ink); }
.chip-action.icon-only { width: 26px; justify-content: center; padding: 0; }

.code { flex: none; display: inline-flex; align-items: center; height: 36px; padding: 0 13px; border: 1px solid rgb(215 249 84 / 35%); border-radius: 9px; background: rgb(215 249 84 / 8%); }
.code strong { color: var(--acid); font: 700 14.5px/1 var(--font-mono); letter-spacing: .14em; }

.idle-hint { flex: 1; min-width: 0; margin: 0; overflow: hidden; color: rgb(251 252 247 / 72%); font-size: 13.5px; text-overflow: ellipsis; white-space: nowrap; }
.spacer { flex: 1; }

/* 停止服务是危险操作，用中性描边降到与「选择文件」匹配的次级权重 */
.power { flex: none; height: 38px; display: inline-flex; align-items: center; gap: 8px; padding: 0 16px; border: 1px solid rgb(251 252 247 / 32%); border-radius: 9px; background: transparent; color: var(--paper); cursor: pointer; font: 680 13.5px/1 var(--font-label); letter-spacing: .02em; transition: background 140ms ease, border-color 140ms ease; }
.power:hover:not(:disabled) { border-color: rgb(251 252 247 / 60%); background: rgb(255 255 255 / 8%); }
/* 启动服务才是这一屏的主行动，独占 acid 实心 */
.power:not(.is-running) { border-color: var(--acid); background: var(--acid); color: var(--ink); font-weight: 720; }
.power:not(.is-running):hover:not(:disabled) { border-color: var(--acid-hi); background: var(--acid-hi); }
.power:disabled { cursor: wait; opacity: .6; }

.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

@media (max-width: 1050px) {
  .device { max-width: 120px; }
  .url { max-width: none; }
}
@media (max-width: 780px) {
  .cmdbar { gap: 10px; padding: 0 14px; }
  .device, .product { display: none; }
  .power { padding: 0 12px; }
}
@media (prefers-reduced-motion: reduce) {
  .spin { animation: none; }
}
</style>
