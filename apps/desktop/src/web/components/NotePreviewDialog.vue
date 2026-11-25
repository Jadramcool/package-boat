<script setup lang="ts">
import { Check, Copy, Download, LoaderCircle, X } from '@lucide/vue'
import { onMounted, onUnmounted, shallowRef } from 'vue'
import { getNoteContent } from '../api'
import type { SharedFile } from '../types'
import { formatBytes } from '@/utils/format'

const props = defineProps<{
  file: SharedFile
}>()

const emit = defineEmits<{
  close: []
}>()

const loading = shallowRef(true)
const content = shallowRef('')
const error = shallowRef('')
const copied = shallowRef(false)

const downloadURL = `/api/files/${encodeURIComponent(props.file.id)}/download`

async function loadContent(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    const response = await getNoteContent(props.file.id)
    content.value = response.content
  }
  catch (cause) {
    error.value = cause instanceof Error ? cause.message : '读取文本失败'
  }
  finally {
    loading.value = false
  }
}

async function copyContent(): Promise<void> {
  if (!content.value)
    return
  error.value = ''
  try {
    await writeClipboard(content.value)
    copied.value = true
    window.setTimeout(() => copied.value = false, 1800)
  }
  catch {
    error.value = '浏览器拒绝访问剪贴板，请手动选择正文复制'
  }
}

async function writeClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return
    }
    catch {
      // 局域网 HTTP 下 Clipboard API 可能因安全上下文限制被拒绝，继续尝试兼容回退。
    }
  }
  const textarea = document.createElement('textarea')
  textarea.value = text
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'fixed'
  textarea.style.inset = '0'
  textarea.style.opacity = '0'
  document.body.appendChild(textarea)
  textarea.select()
  const ok = document.execCommand('copy')
  textarea.remove()
  if (!ok)
    throw new Error('copy failed')
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape')
    emit('close')
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
  void loadContent()
})

onUnmounted(() => window.removeEventListener('keydown', handleKeydown))
</script>

<template>
  <Teleport to="body">
    <div class="dialog-backdrop" @click.self="emit('close')">
      <section class="note-dialog" role="dialog" aria-modal="true" aria-labelledby="note-preview-title">
        <header class="dialog-header">
          <div class="dialog-title">
            <span class="section-index">TEXT / PREVIEW</span>
            <h2 id="note-preview-title">{{ props.file.name }}</h2>
            <p>{{ formatBytes(props.file.size) }} · 文本投递</p>
          </div>
          <button class="icon-button" type="button" aria-label="关闭预览" @click="emit('close')">
            <X :size="18" />
          </button>
        </header>

        <div class="dialog-body">
          <div v-if="loading" class="dialog-state">
            <LoaderCircle class="spin" :size="24" />
            <span>正在读取文本</span>
          </div>
          <p v-else-if="error && !content" class="dialog-state error" role="alert">{{ error }}</p>
          <pre v-else class="note-text">{{ content }}</pre>
        </div>

        <footer class="dialog-footer">
          <p v-if="error && content" class="copy-error" role="alert">{{ error }}</p>
          <p v-else class="dialog-hint">正文以纯文本展示，不会执行其中内容。</p>
          <div class="dialog-actions">
            <button type="button" class="secondary-button" :disabled="loading || !content" @click="copyContent">
              <Check v-if="copied" :size="15" />
              <Copy v-else :size="15" />
              {{ copied ? '已复制' : '复制正文' }}
            </button>
            <a class="primary-button" :href="downloadURL" :download="props.file.name">
              <Download :size="15" />
              下载
            </a>
          </div>
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgb(18 24 16 / 52%);
  backdrop-filter: blur(4px);
}
.note-dialog {
  width: min(680px, 100%);
  max-height: min(78vh, 720px);
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 16px;
  background: var(--surface);
  box-shadow: 0 24px 80px rgb(18 24 16 / 28%);
}
.dialog-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
  padding: 18px 20px 15px;
  border-bottom: 1px solid var(--line);
}
.dialog-title { min-width: 0; }
.section-index {
  color: var(--acid-deep);
  font: 650 9px/1 var(--font-label);
  letter-spacing: .15em;
}
.dialog-title h2 {
  margin: 7px 0 0;
  overflow: hidden;
  color: var(--ink);
  font: 700 20px/1.25 var(--font-display);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dialog-title p {
  margin: 5px 0 0;
  color: var(--muted);
  font: 550 11px/1.3 var(--font-mono);
}
.icon-button {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  flex: none;
  border: 1px solid var(--line-strong);
  border-radius: 9px;
  background: #fff;
  color: var(--muted);
  cursor: pointer;
}
.icon-button:hover { color: var(--ink); border-color: var(--ink); }
.dialog-body {
  min-height: 210px;
  overflow: auto;
  padding: 18px 20px;
  background: var(--dock-0);
}
.dialog-state {
  min-height: 170px;
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 12px;
  color: var(--muted);
  font-size: 13px;
  text-align: center;
}
.dialog-state.error { color: var(--signal-deep); }
.note-text {
  margin: 0;
  color: var(--ink);
  font: 450 13px/1.7 var(--font-mono);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.dialog-footer {
  min-height: 64px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 12px 20px;
  border-top: 1px solid var(--line);
  background: var(--surface);
}
.dialog-hint,
.copy-error {
  margin: 0;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.45;
}
.copy-error { color: var(--signal-deep); }
.dialog-actions { display: flex; align-items: center; gap: 8px; flex: none; }
.secondary-button,
.primary-button {
  height: 34px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 0 12px;
  border: 1px solid var(--line-strong);
  border-radius: 9px;
  cursor: pointer;
  font: 650 11.5px/1 var(--font-label);
  text-decoration: none;
}
.secondary-button { background: #fff; color: var(--ink); }
.secondary-button:hover:not(:disabled) { border-color: var(--ink); }
.secondary-button:disabled { opacity: .5; cursor: not-allowed; }
.primary-button { border-color: var(--ink); background: var(--ink); color: var(--acid); }
.primary-button:hover { background: #343c31; border-color: #343c31; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 560px) {
  .dialog-backdrop { padding: 12px; }
  .note-dialog { max-height: 84vh; border-radius: 13px; }
  .dialog-header, .dialog-body, .dialog-footer { padding-right: 14px; padding-left: 14px; }
  .dialog-footer { align-items: stretch; flex-direction: column; gap: 9px; }
  .dialog-actions { justify-content: flex-end; }
}
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
