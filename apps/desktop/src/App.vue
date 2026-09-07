<script setup lang="ts">
import { defineAsyncComponent } from 'vue'

const desktopProtocols = new Set(['tauri:', 'wails:'])
const query = new URLSearchParams(window.location.search)
const isDesktop = '__TAURI_INTERNALS__' in window
  || desktopProtocols.has(window.location.protocol)
  || ['tauri.localhost', 'wails.localhost'].includes(window.location.hostname)
  || query.get('desktop') === 'preview'

const RuntimeApp = isDesktop
  ? defineAsyncComponent(() => import('./DesktopApp.vue'))
  : defineAsyncComponent(() => import('./web/FileShareApp.vue'))
</script>

<template>
  <RuntimeApp />
</template>
