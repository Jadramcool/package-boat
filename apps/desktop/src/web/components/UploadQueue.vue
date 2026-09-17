<script setup lang="ts">
import { Check, CircleX, LoaderCircle, RotateCcw, X } from '@lucide/vue'
import { formatBytes, formatDuration } from '@/utils/format'
import type { UploadTask } from '../types'

defineProps<{
  uploads: readonly UploadTask[]
  completedCount: number
}>()

const emit = defineEmits<{
  cancel: [id: string]
  retry: [id: string]
  clear: []
}>()

function transferDetail(task: UploadTask): string {
  const parts = task.resumed ? ['断点续传'] : []
  parts.push(`${task.progress}%`)
  parts.push(`${formatBytes(task.speedBytesPerSecond)}/s`)
  parts.push(task.etaSeconds === undefined
    ? '剩余时间计算中'
    : `剩余 ${formatDuration(task.etaSeconds)}`)
  return parts.join(' · ')
}
</script>

<template>
  <section class="upload-queue" aria-labelledby="queue-title">
    <header class="queue-header">
      <div><span class="section-index">TRANSFER / 04</span><h2 id="queue-title">传输队列</h2></div>
      <button v-if="completedCount > 0" class="clear-button" type="button" @click="emit('clear')">清理已完成</button>
    </header>
    <ul class="task-list">
      <li v-for="task in uploads" :key="task.id" class="task-row">
        <div class="task-state" :class="task.status" aria-hidden="true">
          <Check v-if="task.status === 'complete'" :size="16" />
          <CircleX v-else-if="task.status === 'error'" :size="16" />
          <X v-else-if="task.status === 'cancelled'" :size="16" />
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
            <span v-else-if="task.status === 'cancelled'">已取消</span>
            <span v-else>{{ transferDetail(task) }}</span>
          </div>
        </div>
        <button
          v-if="task.status === 'uploading' || task.status === 'queued'"
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
      </li>
    </ul>
  </section>
</template>

<style scoped>
.upload-queue { margin-top: 26px; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 16px; background: var(--surface); box-shadow: 0 8px 24px rgb(31 39 28 / 5%); }
.queue-header { min-height: 74px; padding: 16px 20px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--line); }
.section-index { color: var(--muted); font: 650 8px/1 var(--font-label); letter-spacing: .16em; }
.queue-header h2 { margin: 6px 0 0; font: 700 18px/1 var(--font-display); }
.clear-button { padding: 7px 0; border: 0; border-bottom: 1px solid var(--ink); color: var(--ink); background: transparent; cursor: pointer; font-size: 11px; }
.task-list { margin: 0; padding: 0; list-style: none; }
.task-row { padding: 16px 20px; display: grid; grid-template-columns: auto 1fr auto; align-items: center; gap: 14px; border-bottom: 1px solid var(--line); }
.task-row:last-child { border-bottom: 0; }
.task-state { width: 30px; height: 30px; display: grid; place-items: center; border: 1px solid var(--line-strong); border-radius: 9px; color: var(--muted); }
.task-state.uploading svg { animation: spin 1.2s linear infinite; }
.task-state.complete { border-color: var(--ink); color: var(--ink); background: var(--acid); }
.task-state.error { border-color: var(--signal); color: var(--signal); }
.task-main { min-width: 0; }
.task-line { display: flex; justify-content: space-between; gap: 18px; font-size: 12px; }
.task-name { overflow: hidden; color: var(--ink); text-overflow: ellipsis; white-space: nowrap; font-weight: 650; }
.task-size { flex: none; color: var(--muted); font-family: var(--font-mono); font-size: 10px; }
.progress-track { height: 6px; margin-top: 9px; overflow: hidden; border-radius: 999px; background: var(--line); }
.progress-track span { height: 100%; display: block; border-radius: 999px; background: linear-gradient(90deg, var(--acid), var(--acid-hi)); transition: width .12s linear; }
.task-detail { min-height: 13px; margin-top: 7px; color: var(--muted); font: 600 10px/1.3 var(--font-label); letter-spacing: .04em; }
.task-error { color: var(--signal-deep); }
.task-action { width: 30px; height: 30px; display: grid; place-items: center; border: 1px solid transparent; border-radius: 8px; color: var(--muted); background: transparent; cursor: pointer; }
.task-action:hover { border-color: var(--line-strong); color: var(--signal); }
@keyframes spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .task-state.uploading svg { animation: none; } }
</style>
