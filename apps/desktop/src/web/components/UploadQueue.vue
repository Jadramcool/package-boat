<script setup lang="ts">
import { Check, CircleX, History, LoaderCircle, Pause, Play, RotateCcw, X } from '@lucide/vue'
import { formatBytes, formatDate, formatDuration } from '@/utils/format'
import type { TransferHistoryEntry, UploadTask } from '../types'

defineProps<{
  uploads: readonly UploadTask[]
  history: readonly TransferHistoryEntry[]
  completedCount: number
  concurrency: number
}>()

const emit = defineEmits<{
  cancel: [id: string]
  pause: [id: string]
  resume: [id: string]
  retry: [id: string]
  clear: []
  clearHistory: []
  'update:concurrency': [value: number]
}>()

const concurrencyOptions = [1, 2, 3, 4, 6, 8]

function onConcurrencyChange(event: Event): void {
  const target = event.target as HTMLSelectElement
  emit('update:concurrency', Number(target.value))
}

function transferDetail(task: UploadTask): string {
  const parts = task.resumed ? ['断点续传'] : []
  parts.push(`${task.progress}%`)
  parts.push(`${formatBytes(task.speedBytesPerSecond)}/s`)
  parts.push(task.etaSeconds === undefined
    ? '剩余时间计算中'
    : `剩余 ${formatDuration(task.etaSeconds)}`)
  return parts.join(' · ')
}

function historyLabel(entry: TransferHistoryEntry): string {
  if (entry.status === 'complete')
    return '已完成'
  if (entry.status === 'cancelled')
    return '已取消'
  return entry.error ?? '失败'
}
</script>

<template>
  <section class="upload-queue" aria-labelledby="queue-title">
    <header class="queue-header">
      <div>
        <span class="section-index">TRANSFER / 04</span>
        <h2 id="queue-title">传输队列</h2>
      </div>
      <div class="queue-actions">
        <label class="concurrency-field">
          <span>并行</span>
          <select
            :value="concurrency"
            aria-label="同时上传文件数"
            @change="onConcurrencyChange"
          >
            <option v-for="option in concurrencyOptions" :key="option" :value="option">
              {{ option }}
            </option>
          </select>
        </label>
        <button v-if="completedCount > 0" class="clear-button" type="button" @click="emit('clear')">
          清理已完成
        </button>
      </div>
    </header>
    <ul v-if="uploads.length > 0" class="task-list">
      <li v-for="task in uploads" :key="task.id" class="task-row">
        <div class="task-state" :class="task.status" aria-hidden="true">
          <Check v-if="task.status === 'complete'" :size="16" />
          <CircleX v-else-if="task.status === 'error'" :size="16" />
          <X v-else-if="task.status === 'cancelled'" :size="16" />
          <Pause v-else-if="task.status === 'paused'" :size="16" />
          <LoaderCircle v-else :size="16" />
        </div>
        <div class="task-main">
          <div class="task-line">
            <span class="task-name" :title="task.file.name">{{ task.file.name }}</span>
            <span class="task-size">{{ formatBytes(task.file.size) }}</span>
          </div>
          <div class="progress-track"><span :style="{ width: `${task.progress}%` }" /></div>
          <div class="task-detail">
            <span v-if="task.error" class="task-error">{{ task.error }}</span>
            <span v-else-if="task.status === 'complete'">已投递</span>
            <span v-else-if="task.status === 'queued'">等待中</span>
            <span v-else-if="task.status === 'paused'">已暂停 · 进度 {{ task.progress }}%</span>
            <span v-else-if="task.status === 'cancelled'">已取消</span>
            <span v-else>{{ transferDetail(task) }}</span>
          </div>
        </div>
        <div class="task-actions">
          <button
            v-if="task.status === 'uploading' || task.status === 'queued'"
            class="task-action"
            type="button"
            title="暂停上传"
            aria-label="暂停上传"
            @click="emit('pause', task.id)"
          ><Pause :size="15" /></button>
          <button
            v-else-if="task.status === 'paused'"
            class="task-action"
            type="button"
            title="继续上传"
            aria-label="继续上传"
            @click="emit('resume', task.id)"
          ><Play :size="15" /></button>
          <button
            v-if="task.status === 'uploading' || task.status === 'queued' || task.status === 'paused'"
            class="task-action"
            type="button"
            title="取消上传"
            aria-label="取消上传"
            @click="emit('cancel', task.id)"
          ><X :size="16" /></button>
          <button
            v-else-if="task.status === 'error' || task.status === 'cancelled'"
            class="task-action"
            type="button"
            title="重新上传"
            aria-label="重新上传"
            @click="emit('retry', task.id)"
          ><RotateCcw :size="15" /></button>
        </div>
      </li>
    </ul>
    <p v-else class="queue-empty">暂无进行中的传输</p>

    <section v-if="history.length > 0" class="history-block" aria-labelledby="history-title">
      <header class="history-header">
        <div class="history-heading">
          <History :size="14" aria-hidden="true" />
          <h3 id="history-title">传输历史</h3>
          <span class="history-count">{{ history.length }} 条</span>
        </div>
        <button class="clear-button" type="button" @click="emit('clearHistory')">清空历史</button>
      </header>
      <ul class="history-list">
        <li v-for="entry in history" :key="entry.id" class="history-row">
          <span class="history-status" :class="entry.status" aria-hidden="true">
            <Check v-if="entry.status === 'complete'" :size="12" />
            <CircleX v-else-if="entry.status === 'error'" :size="12" />
            <X v-else :size="12" />
          </span>
          <span class="history-name" :title="entry.name">{{ entry.name }}</span>
          <span class="history-meta">{{ formatBytes(entry.size) }}</span>
          <span class="history-meta">{{ formatDate(entry.finishedAt) }}</span>
          <span class="history-result" :class="entry.status">{{ historyLabel(entry) }}</span>
        </li>
      </ul>
    </section>
  </section>
</template>

<style scoped>
.upload-queue { overflow: hidden; border: 1px solid var(--line-strong); border-radius: 14px; background: var(--surface); box-shadow: 0 8px 24px rgb(31 39 28 / 5%); }
.queue-header { min-height: 56px; padding: 12px 16px; display: flex; align-items: center; justify-content: space-between; gap: 16px; border-bottom: 1px solid var(--line); }
.section-index { color: var(--muted); font: 650 8px/1 var(--font-label); letter-spacing: .16em; }
.queue-header h2 { margin: 6px 0 0; font: 700 18px/1 var(--font-display); }
.queue-actions { display: flex; align-items: center; gap: 14px; }
.concurrency-field { display: flex; align-items: center; gap: 8px; color: var(--muted); font: 600 11px/1 var(--font-label); letter-spacing: .06em; }
.concurrency-field select { height: 28px; padding: 0 8px; border: 1px solid var(--line-strong); border-radius: 8px; color: var(--ink); background: var(--surface); font: 650 12px/1 var(--font-mono); cursor: pointer; }
.concurrency-field select:focus-visible { outline: 2px solid var(--info); outline-offset: 1px; }
.clear-button { padding: 7px 0; border: 0; border-bottom: 1px solid var(--ink); color: var(--ink); background: transparent; cursor: pointer; font-size: 11px; }
.clear-button:hover { color: var(--signal); border-color: var(--signal); }
.queue-empty { margin: 0; padding: 18px 20px; color: var(--muted); font-size: 12px; border-bottom: 1px solid var(--line); }
.task-list { margin: 0; padding: 0; list-style: none; }
.task-row { padding: 14px 16px; display: grid; grid-template-columns: auto 1fr auto; align-items: center; gap: 12px; border-bottom: 1px solid var(--line); }
.task-row:last-child { border-bottom: 0; }
.task-state { width: 30px; height: 30px; display: grid; place-items: center; border: 1px solid var(--line-strong); border-radius: 9px; color: var(--muted); }
.task-state.uploading svg { animation: spin 1.2s linear infinite; }
.task-state.complete { border-color: var(--ink); color: var(--ink); background: var(--acid); }
.task-state.error { border-color: var(--signal); color: var(--signal); }
.task-state.paused { border-color: var(--info); color: var(--info); }
.task-main { min-width: 0; }
.task-line { display: flex; justify-content: space-between; gap: 18px; font-size: 12px; }
.task-name { overflow: hidden; color: var(--ink); text-overflow: ellipsis; white-space: nowrap; font-weight: 650; }
.task-size { flex: none; color: var(--muted); font-family: var(--font-mono); font-size: 10px; }
.progress-track { height: 6px; margin-top: 9px; overflow: hidden; border-radius: 999px; background: var(--line); }
.progress-track span { height: 100%; display: block; border-radius: 999px; background: linear-gradient(90deg, var(--acid), var(--acid-hi)); transition: width .12s linear; }
.task-row:has(.task-state.paused) .progress-track span { background: var(--info); }
.task-detail { min-height: 13px; margin-top: 7px; color: var(--muted); font: 600 10px/1.3 var(--font-label); letter-spacing: .04em; }
.task-error { color: var(--signal-deep); }
.task-actions { display: flex; align-items: center; gap: 2px; }
.task-action { width: 30px; height: 30px; display: grid; place-items: center; border: 1px solid transparent; border-radius: 8px; color: var(--muted); background: transparent; cursor: pointer; }
.task-action:hover { border-color: var(--line-strong); color: var(--ink); }
.task-actions .task-action:last-child:hover { color: var(--signal); }
.history-block { border-top: 1px solid var(--line); background: color-mix(in srgb, var(--line) 28%, var(--surface)); }
.history-header { min-height: 44px; padding: 10px 20px; display: flex; align-items: center; justify-content: space-between; gap: 12px; border-bottom: 1px solid var(--line); }
.history-heading { display: flex; align-items: center; gap: 8px; color: var(--ink); }
.history-heading h3 { margin: 0; font: 700 13px/1 var(--font-display); }
.history-count { color: var(--muted); font: 600 10px/1 var(--font-label); letter-spacing: .06em; }
.history-list { margin: 0; padding: 0; list-style: none; max-height: 220px; overflow-y: auto; }
.history-row { padding: 10px 20px; display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto auto; align-items: center; gap: 12px; border-bottom: 1px solid var(--line); font-size: 12px; }
.history-row:last-child { border-bottom: 0; }
.history-status { width: 22px; height: 22px; display: grid; place-items: center; border-radius: 6px; border: 1px solid var(--line-strong); color: var(--muted); }
.history-status.complete { border-color: var(--ink); color: var(--ink); background: var(--acid); }
.history-status.error { border-color: var(--signal); color: var(--signal); }
.history-name { overflow: hidden; color: var(--ink); text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
.history-meta { color: var(--muted); font: 600 10px/1 var(--font-mono); white-space: nowrap; }
.history-result { min-width: 48px; text-align: right; color: var(--muted); font: 650 10px/1 var(--font-label); letter-spacing: .04em; white-space: nowrap; }
.history-result.complete { color: var(--acid-deep); }
.history-result.error { color: var(--signal-deep); }
@keyframes spin { to { transform: rotate(360deg); } }
@media (max-width: 560px) {
  .queue-header { align-items: flex-start; flex-direction: column; }
  .history-row { grid-template-columns: auto minmax(0, 1fr) auto; grid-template-areas: 'status name result' 'status meta meta'; }
  .history-status { grid-area: status; }
  .history-name { grid-area: name; }
  .history-result { grid-area: result; }
  .history-meta { grid-area: meta; justify-self: start; }
  .history-meta + .history-meta { display: none; }
}
@media (prefers-reduced-motion: reduce) { .task-state.uploading svg { animation: none; } }
</style>
