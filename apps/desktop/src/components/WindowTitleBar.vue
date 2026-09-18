<script setup lang="ts">
import { Copy, Minus, Pin, PinOff, Square, X } from '@lucide/vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { onMounted, onUnmounted, ref } from 'vue'

const props = defineProps<{ alwaysOnTop: boolean }>()
const emit = defineEmits<{ toggleAlwaysOnTop: [] }>()

// 无边框窗口的专属标题栏：整条可拖拽（按钮除外），双击最大化
const win = getCurrentWindow()
const maximized = ref(false)
let cancelResize: (() => void) | undefined

onMounted(async () => {
  try {
    maximized.value = await win.isMaximized()
    cancelResize = await win.onResized(async () => {
      maximized.value = await win.isMaximized()
    })
  }
  catch {
    // 非 Tauri 环境（浏览器预览）下忽略
  }
})

onUnmounted(() => cancelResize?.())

function minimizeWindow() {
  void win.minimize()
}

function toggleMaximize() {
  void win.toggleMaximize()
}

function closeWindow() {
  void win.close()
}
</script>

<template>
  <div class="window-titlebar" data-tauri-drag-region="deep">
    <div class="titlebar-brand" aria-hidden="true">
      <span class="titlebar-title">PacketBoat</span>
    </div>
    <div class="titlebar-actions">
      <button
        class="titlebar-pin"
        :class="{ active: props.alwaysOnTop }"
        type="button"
        :title="props.alwaysOnTop ? '取消窗口置顶' : '窗口置顶'"
        :aria-label="props.alwaysOnTop ? '取消窗口置顶' : '窗口置顶'"
        :aria-pressed="props.alwaysOnTop"
        @click="emit('toggleAlwaysOnTop')"
      >
        <PinOff v-if="props.alwaysOnTop" :size="15" />
        <Pin v-else :size="15" />
      </button>
      <div class="window-controls">
        <button type="button" title="最小化" aria-label="最小化" @click="minimizeWindow">
          <Minus :size="16" />
        </button>
        <button type="button" :title="maximized ? '还原' : '最大化'" :aria-label="maximized ? '还原' : '最大化'" @click="toggleMaximize">
          <Copy v-if="maximized" :size="13" />
          <Square v-else :size="12" />
        </button>
        <button type="button" class="close" title="关闭（最小化到托盘）" aria-label="关闭" @click="closeWindow">
          <X :size="16" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 方案 A：标题栏只做系统 chrome，去掉品牌 L 标；
   品牌与产品信息只出现在下方命令条，避免同一窗口重复两次品牌。 */
.window-titlebar {
  flex: none;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: #161c14;
  color: var(--paper);
  user-select: none;
  border-bottom: 1px solid rgb(251 252 247 / 8%);
}
.titlebar-brand {
  display: flex;
  align-items: center;
  min-width: 0;
  padding: 0 16px;
}
.titlebar-title {
  overflow: hidden;
  color: rgb(251 252 247 / 48%);
  font: 600 11.5px/1 var(--font-label);
  letter-spacing: 0.12em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.titlebar-actions {
  display: flex;
  align-items: stretch;
  height: 100%;
}
.titlebar-pin {
  width: 44px;
  display: grid;
  place-items: center;
  border: 0;
  background: transparent;
  color: rgb(251 252 247 / 62%);
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease;
}
.titlebar-pin:hover:not(.active) {
  background: rgb(255 255 255 / 8%);
  color: var(--paper);
}
.titlebar-pin.active {
  background: var(--acid);
  color: var(--ink);
}
.titlebar-pin.active:hover {
  background: #e4ff7c;
}
.window-controls {
  display: flex;
  align-items: stretch;
  height: 100%;
}
.window-controls button {
  width: 46px;
  display: grid;
  place-items: center;
  border: 0;
  background: transparent;
  color: rgb(251 252 247 / 62%);
  cursor: default;
  transition: background 120ms ease, color 120ms ease;
}
.window-controls button:hover {
  background: rgb(255 255 255 / 8%);
  color: var(--paper);
}
.window-controls button.close:hover {
  background: var(--signal);
  color: #fff;
}
</style>
