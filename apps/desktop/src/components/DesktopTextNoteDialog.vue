<script setup lang="ts">
import { AlertCircle, ClipboardPaste, LoaderCircle, Send, X } from '@lucide/vue'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import { computed, nextTick, onMounted, onUnmounted, shallowRef, useTemplateRef } from 'vue'
import { formatBytes } from '@/utils/format'
import { MAX_NOTE_BYTES, utf8ByteLength } from '@/utils/note'

const props = defineProps<{
  busy: boolean
  error?: string
}>()

const emit = defineEmits<{
  submit: [title: string, content: string]
  close: []
}>()

const title = defineModel<string>('title', { default: '' })
const content = defineModel<string>('content', { default: '' })
const pasteMessage = shallowRef('')
const bodyInput = useTemplateRef<HTMLTextAreaElement>('bodyInput')

const byteLength = computed(() => utf8ByteLength(content.value))
const overLimit = computed(() => byteLength.value > MAX_NOTE_BYTES)
const canSubmit = computed(() =>
  !props.busy
  && content.value.trim().length > 0
  && !overLimit.value,
)

function submit(): void {
  if (!canSubmit.value)
    return
  pasteMessage.value = ''
  emit('submit', title.value, content.value)
}

function close(): void {
  if (!props.busy)
    emit('close')
}

async function pasteClipboard(): Promise<void> {
  if (props.busy)
    return
  try {
    const text = await readText()
    if (!text) {
      pasteMessage.value = '剪贴板是空的'
      return
    }
    content.value = content.value
      ? `${content.value.replace(/\n+$/, '')}\n${text}`
      : text
    pasteMessage.value = '已从剪贴板追加正文'
  }
  catch {
    pasteMessage.value = '无法读取剪贴板，请直接在正文框粘贴'
  }
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    return
  }
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault()
    submit()
  }
}

onMounted(async () => {
  window.addEventListener('keydown', handleKeydown)
  await nextTick()
  bodyInput.value?.focus()
})

onUnmounted(() => window.removeEventListener('keydown', handleKeydown))
</script>

<template>
  <Teleport to="body">
    <div class="note-backdrop" @click.self="close">
      <section class="note-dialog" role="dialog" aria-modal="true" aria-labelledby="desktop-note-title">
        <header class="note-header">
          <div class="note-heading">
            <span class="note-kicker">TEXT / DELIVERY</span>
            <h2 id="desktop-note-title">投递文本</h2>
            <p>保存为接收目录中的 .txt，并立即加入共享清单。</p>
          </div>
          <button class="icon-button" type="button" :disabled="props.busy" aria-label="关闭" @click="close">
            <X :size="18" />
          </button>
        </header>

        <form class="note-form" @submit.prevent="submit">
          <label class="note-field">
            <span>标题（可选）</span>
            <input
              v-model="title"
              type="text"
              maxlength="80"
              placeholder="例如：会议纪要"
              :disabled="props.busy"
            >
          </label>

          <label class="note-field body-field">
            <span>正文</span>
            <textarea
              ref="bodyInput"
              v-model="content"
              rows="8"
              placeholder="粘贴链接、口令、一段说明或整段剪贴板内容…"
              :disabled="props.busy"
              aria-describedby="desktop-note-limit"
            />
          </label>

          <div class="note-meta">
            <span id="desktop-note-limit" class="byte-counter" :class="{ over: overLimit }">
              {{ formatBytes(byteLength) }} / 64 KB
              <template v-if="overLimit"> · 超出限制</template>
            </span>
            <span class="shortcut">Ctrl + Enter 投递</span>
          </div>

          <p
            v-if="props.error || pasteMessage"
            class="note-status"
            :class="{ error: props.error }"
            :role="props.error ? 'alert' : 'status'"
          >
            <AlertCircle v-if="props.error" :size="14" aria-hidden="true" />
            {{ props.error || pasteMessage }}
          </p>

          <footer class="note-actions">
            <button type="button" class="paste-button" :disabled="props.busy" @click="pasteClipboard">
              <ClipboardPaste :size="15" />
              粘贴剪贴板
            </button>
            <div class="primary-actions">
              <button type="button" class="cancel-button" :disabled="props.busy" @click="close">
                取消
              </button>
              <button type="submit" class="send-button" :disabled="!canSubmit">
                <LoaderCircle v-if="props.busy" class="spin" :size="15" />
                <Send v-else :size="15" />
                {{ props.busy ? '投递中…' : '投递文本' }}
              </button>
            </div>
          </footer>
        </form>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.note-backdrop {
  position: fixed;
  inset: 0;
  z-index: 90;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgb(18 24 16 / 52%);
  backdrop-filter: blur(4px);
}
.note-dialog {
  width: min(620px, 100%);
  max-height: min(84vh, 760px);
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 16px;
  background: var(--surface);
  box-shadow: 0 24px 80px rgb(18 24 16 / 28%);
}
.note-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
  padding: 18px 20px 15px;
  border-bottom: 1px solid var(--line);
}
.note-heading { min-width: 0; }
.note-kicker {
  color: var(--acid-deep);
  font: 650 9px/1 var(--font-label);
  letter-spacing: .15em;
}
.note-heading h2 {
  margin: 7px 0 0;
  color: var(--ink);
  font: 700 21px/1.2 var(--font-display);
}
.note-heading p {
  margin: 5px 0 0;
  color: var(--muted);
  font-size: 11.5px;
  line-height: 1.45;
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
.icon-button:hover:not(:disabled) { border-color: var(--ink); color: var(--ink); }
.icon-button:disabled { opacity: .5; cursor: wait; }
.note-form {
  min-height: 0;
  display: grid;
  gap: 12px;
  padding: 16px 20px 18px;
  overflow-y: auto;
}
.note-field { display: grid; gap: 6px; }
.note-field > span { color: var(--muted); font: 600 11.5px/1 var(--font-label); }
.note-field input,
.note-field textarea {
  width: 100%;
  border: 1px solid var(--line-strong);
  border-radius: 9px;
  background: #fff;
  color: var(--ink);
  font: 450 13px/1.55 var(--font-mono);
  padding: 9px 11px;
}
.note-field textarea { min-height: 176px; resize: vertical; }
.note-field input:focus-visible,
.note-field textarea:focus-visible { outline: 2px solid var(--info); outline-offset: -1px; }
.note-field input:disabled,
.note-field textarea:disabled { opacity: .68; }
.note-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.byte-counter,
.shortcut { color: var(--muted); font: 600 10.5px/1.3 var(--font-mono); }
.byte-counter.over { color: var(--signal-deep); }
.note-status {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  color: var(--acid-deep);
  font-size: 11.5px;
  line-height: 1.45;
}
.note-status.error { color: var(--signal-deep); }
.note-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 4px;
}
.paste-button,
.cancel-button,
.send-button {
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 0 13px;
  border: 1px solid var(--line-strong);
  border-radius: 9px;
  cursor: pointer;
  font: 650 12px/1 var(--font-label);
}
.paste-button { background: #fff; color: var(--ink); }
.paste-button:hover:not(:disabled) { border-color: var(--ink); }
.primary-actions { display: flex; align-items: center; gap: 8px; }
.cancel-button { background: transparent; color: var(--muted); }
.cancel-button:hover:not(:disabled) { border-color: var(--ink); color: var(--ink); }
.send-button { border-color: var(--ink); background: var(--ink); color: var(--acid); }
.send-button:hover:not(:disabled) { border-color: #343c31; background: #343c31; }
.paste-button:disabled,
.cancel-button:disabled,
.send-button:disabled { opacity: .5; cursor: not-allowed; }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 560px) {
  .note-backdrop { padding: 12px; }
  .note-dialog { max-height: 88vh; border-radius: 13px; }
  .note-header,
  .note-form { padding-right: 14px; padding-left: 14px; }
  .note-actions { align-items: stretch; flex-direction: column-reverse; }
  .paste-button { width: 100%; }
  .primary-actions { justify-content: flex-end; }
  .shortcut { display: none; }
}
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
