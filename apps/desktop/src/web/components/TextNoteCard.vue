<script setup lang="ts">
import { ClipboardPaste, LoaderCircle, Send } from '@lucide/vue'
import { computed, shallowRef } from 'vue'
import { formatBytes } from '@/utils/format'
import { MAX_NOTE_BYTES, utf8ByteLength } from '@/utils/note'

const props = defineProps<{
  disabled: boolean
  busy?: boolean
  message?: string
  messageTone?: 'success' | 'error' | ''
}>()

const emit = defineEmits<{
  send: [title: string, content: string]
}>()

const title = defineModel<string>('title', { default: '' })
const content = defineModel<string>('content', { default: '' })
const pasteMessage = shallowRef('')
const byteLength = computed(() => utf8ByteLength(content.value))
const overLimit = computed(() => byteLength.value > MAX_NOTE_BYTES)
const canSend = computed(() =>
  !props.disabled
  && !props.busy
  && content.value.trim().length > 0
  && !overLimit.value,
)

function submit(): void {
  if (!canSend.value)
    return
  pasteMessage.value = ''
  emit('send', title.value, content.value)
}

async function pasteClipboard(): Promise<void> {
  if (props.disabled || props.busy)
    return
  try {
    const text = await navigator.clipboard.readText()
    if (!text) {
      pasteMessage.value = '剪贴板是空的'
      return
    }
    content.value = content.value
      ? `${content.value.replace(/\n+$/, '')}\n${text}`
      : text
    pasteMessage.value = '已粘贴到正文'
  }
  catch {
    pasteMessage.value = '浏览器拒绝读取剪贴板，请手动粘贴'
  }
}
</script>

<template>
  <section class="note-card" aria-labelledby="note-title">
    <header class="note-header">
      <div>
        <span class="section-index">NOTE</span>
        <h2 id="note-title">文本 / 剪贴板投递</h2>
      </div>
      <button
        type="button"
        class="paste-button"
        :disabled="disabled"
        title="从剪贴板粘贴到正文"
        @click="pasteClipboard"
      >
        <ClipboardPaste :size="14" aria-hidden="true" />
        粘贴剪贴板
      </button>
    </header>

    <p class="note-hint">
      短文本写入接收目录并进入共享清单（上限 64KB），主机端可直接复制或下载。
    </p>
    <p
      v-if="props.message || pasteMessage"
      class="note-message"
      :class="pasteMessage ? 'info' : props.messageTone"
      :role="!pasteMessage && props.messageTone === 'error' ? 'alert' : 'status'"
    >
      {{ pasteMessage || props.message }}
    </p>

    <label class="note-field">
      <span>标题（可选）</span>
      <input
        v-model="title"
        type="text"
        maxlength="80"
        placeholder="例如：会议纪要"
        :disabled="disabled"
      >
    </label>

    <label class="note-field">
      <span>正文</span>
      <textarea
        v-model="content"
        rows="4"
        placeholder="粘贴链接、口令或一段说明…"
        :disabled="disabled"
      />
      <span class="byte-counter" :class="{ over: overLimit }">
        {{ formatBytes(byteLength) }} / 64 KB
        <template v-if="overLimit"> · 超出限制，请缩短正文</template>
      </span>
    </label>

    <div class="note-actions">
      <button
        type="button"
        class="send-button"
        :disabled="!canSend"
        @click="submit"
      >
        <LoaderCircle v-if="busy" class="spin" :size="15" aria-hidden="true" />
        <Send v-else :size="15" aria-hidden="true" />
        {{ busy ? '投递中…' : '投递文本' }}
      </button>
      <span v-if="disabled" class="offline-note">等待网络重新连接</span>
    </div>
  </section>
</template>

<style scoped>
.note-card {
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 14px;
  background: var(--surface);
  padding: 14px 16px 16px;
  display: grid;
  gap: 10px;
}

.note-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.section-index {
  display: block;
  color: var(--muted);
  font: 650 10px/1 var(--font-label);
  letter-spacing: 0.14em;
  margin-bottom: 6px;
}

.note-header h2 {
  margin: 0;
  color: var(--ink);
  font: 700 15px/1.25 var(--font-display);
}

.paste-button {
  flex: none;
  height: 30px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  background: #fff;
  color: var(--ink);
  cursor: pointer;
  font: 650 11.5px/1 var(--font-label);
}

.paste-button:hover:not(:disabled) {
  border-color: var(--ink);
  background: var(--ink);
  color: var(--paper);
}

.paste-button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.note-hint,
.note-message {
  margin: 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.5;
}

.note-message.info {
  color: var(--acid-deep);
}

.note-message.success {
  color: var(--acid-deep);
}

.note-message.error {
  color: var(--signal-deep);
}

.byte-counter {
  justify-self: end;
  color: var(--muted);
  font: 600 10.5px/1.3 var(--font-mono);
}

.byte-counter.over {
  color: var(--signal-deep);
}

.note-field {
  display: grid;
  gap: 6px;
}

.note-field > span {
  color: var(--muted);
  font: 600 11.5px/1 var(--font-label);
}

.note-field input,
.note-field textarea {
  width: 100%;
  border: 1px solid var(--line-strong);
  border-radius: 8px;
  background: #fff;
  color: var(--ink);
  font: 450 13px/1.5 var(--font-mono);
  padding: 8px 10px;
  resize: vertical;
}

.note-field input:focus-visible,
.note-field textarea:focus-visible {
  outline: 2px solid var(--info);
  outline-offset: -1px;
}

.note-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.send-button {
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 0 16px;
  border: 1px solid var(--ink);
  border-radius: 10px;
  background: var(--ink);
  color: var(--acid);
  cursor: pointer;
  font: 680 13px/1.2 var(--font-label);
}

.send-button:hover:not(:disabled) {
  background: #343c31;
  border-color: #343c31;
}

.send-button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.offline-note {
  color: var(--muted);
  font-size: 12px;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 560px) {
  .note-header {
    flex-direction: column;
  }
}
</style>
