<script setup lang="ts">
import { LogOut, Radio } from '@lucide/vue'

defineProps<{
  deviceName: string
  online: boolean
  version: string
}>()

const emit = defineEmits<{
  signOut: []
}>()
</script>

<template>
  <header class="app-header">
    <div class="header-inner">
      <a class="brand" href="/" aria-label="LANE 首页">
        <span class="brand-glyph" aria-hidden="true">L</span>
        <span class="brand-word">LANE</span>
        <span class="brand-sub">局域网投递站</span>
      </a>
      <div class="device-status">
        <span class="status-pill" :class="{ offline: !online }">
          <Radio :size="13" aria-hidden="true" /> {{ online ? '在线' : '重连中' }}
        </span>
        <span class="device-name" :title="deviceName">{{ deviceName }}</span>
        <span v-if="version" class="version">v{{ version }}</span>
        <button type="button" class="sign-out" title="断开此设备" aria-label="断开此设备" @click="emit('signOut')">
          <LogOut :size="17" aria-hidden="true" />
        </button>
      </div>
    </div>
  </header>
</template>

<style scoped>
.app-header { position: sticky; top: 0; z-index: 20; border-bottom: 1px solid var(--line-strong); background: color-mix(in srgb, var(--paper) 93%, transparent); backdrop-filter: blur(12px); }
.header-inner { width: min(1180px, calc(100% - 40px)); min-height: 68px; margin: 0 auto; display: flex; align-items: center; justify-content: space-between; }
.brand { min-width: 0; display: flex; align-items: center; color: var(--ink); text-decoration: none; }
.brand-glyph { width: 34px; height: 34px; display: grid; place-items: center; border: 1px solid var(--ink); background: var(--acid); font: 750 17px/1 var(--font-display); }
.brand-word { margin-left: 10px; font: 750 18px/1 var(--font-display); letter-spacing: .08em; }
.brand-sub { margin-left: 13px; padding-left: 13px; border-left: 1px solid var(--line-strong); color: var(--muted); font-size: 10px; letter-spacing: .08em; }
.device-status { min-width: 0; display: flex; align-items: center; gap: 10px; }
.status-pill { padding: 6px 8px; display: inline-flex; align-items: center; gap: 5px; color: var(--ink); background: var(--acid); font: 650 9px/1 var(--font-label); letter-spacing: .08em; }
.status-pill.offline { color: #74220d; background: var(--signal-soft); }
.device-name { max-width: 200px; overflow: hidden; color: var(--ink); text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.version { color: var(--muted); font: 600 9px/1 var(--font-label); }
.sign-out { width: 34px; height: 34px; display: grid; place-items: center; border: 1px solid transparent; color: var(--muted); background: transparent; cursor: pointer; transition: border-color .15s, color .15s, background .15s; }
.sign-out:hover { border-color: var(--line-strong); color: var(--signal); background: var(--surface); }
@media (max-width: 600px) {
  .header-inner { width: calc(100% - 24px); }
  .brand-sub, .version, .device-name { display: none; }
}
</style>
