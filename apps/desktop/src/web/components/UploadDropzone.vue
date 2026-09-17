<script setup lang="ts">
import { ArrowUpRight, CloudUpload, Files } from '@lucide/vue'
import { shallowRef, useTemplateRef } from 'vue'

const props = defineProps<{
  disabled: boolean
  maxSizeLabel: string
}>()

const emit = defineEmits<{
  selectFiles: [files: File[]]
}>()

const dragging = shallowRef(false)
const fileInput = useTemplateRef<HTMLInputElement>('fileInput')

function chooseFiles(): void {
  if (!props.disabled)
    fileInput.value?.click()
}

function selectFiles(files: FileList | null): void {
  if (!files || props.disabled)
    return
  emit('selectFiles', Array.from(files))
}

function handleChange(event: Event): void {
  const input = event.target as HTMLInputElement
  selectFiles(input.files)
  input.value = ''
}

function handleDragEnter(): void {
  if (!props.disabled)
    dragging.value = true
}

function handleDragLeave(event: DragEvent): void {
  const zone = event.currentTarget as HTMLElement
  if (!event.relatedTarget || !zone.contains(event.relatedTarget as Node))
    dragging.value = false
}

function handleDrop(event: DragEvent): void {
  dragging.value = false
  selectFiles(event.dataTransfer?.files ?? null)
}
</script>

<template>
  <section
    class="dropzone"
    :class="{ dragging, disabled }"
    aria-labelledby="upload-title"
    @dragenter.prevent="handleDragEnter"
    @dragover.prevent
    @dragleave="handleDragLeave"
    @drop.prevent="handleDrop"
  >
    <input ref="fileInput" class="visually-hidden" type="file" multiple @change="handleChange">
    <div class="drop-index">DROP / 03</div>
    <div class="upload-icon" aria-hidden="true"><CloudUpload :size="30" :stroke-width="1.6" /></div>
    <div class="drop-copy">
      <h2 id="upload-title">把文件投递到这里</h2>
      <p>{{ disabled ? '正在等待网络重新连接' : '拖拽文件到此区域，或从设备中选择；支持多文件并行传输。' }}</p>
      <div class="upload-meta">
        <span><Files :size="14" /> 任意文件类型</span>
        <span>单个不超过 {{ maxSizeLabel }}</span>
      </div>
    </div>
    <button class="choose-button" type="button" :disabled="disabled" @click="chooseFiles">
      选择文件 <ArrowUpRight :size="18" aria-hidden="true" />
    </button>
  </section>
</template>

<style scoped>
.dropzone { min-height: 96px; padding: 18px 22px; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 20px; position: relative; overflow: hidden; border: 1px solid var(--line-strong); border-radius: 16px; background: linear-gradient(90deg, transparent 49.5%, var(--line) 50%, transparent 50.5%) 0 0 / 24px 24px, linear-gradient(transparent 49.5%, var(--line) 50%, transparent 50.5%) 0 0 / 24px 24px, var(--surface); transition: border-color .16s, transform .16s, box-shadow .16s; }
.dropzone.dragging { transform: translate(-4px, -4px); border-color: var(--acid-deep); box-shadow: 8px 8px 0 var(--acid), 9px 9px 0 var(--ink); background-color: var(--acid-wash); }
.dropzone.disabled { opacity: .58; }
.drop-index { position: absolute; top: 9px; left: 14px; color: var(--muted); font: 650 9px/1 var(--font-label); letter-spacing: .18em; }
.upload-icon { width: 46px; height: 46px; display: grid; place-items: center; border: 1px solid var(--ink); border-radius: 50% 50% 10px; color: var(--ink); background: var(--acid); }
.drop-copy { min-width: 0; }
.drop-copy h2 { margin: 0; color: var(--ink); font: 700 clamp(17px, 2.1vw, 21px)/1.15 var(--font-display); letter-spacing: -.03em; }
.drop-copy > p { margin: 7px 0 0; color: var(--muted); font-size: 12.5px; line-height: 1.6; }
.upload-meta { margin-top: 9px; display: flex; flex-wrap: wrap; gap: 14px; color: var(--muted); font: 600 10px/1 var(--font-label); letter-spacing: .06em; }
.upload-meta span { display: inline-flex; align-items: center; gap: 6px; }
.choose-button { align-self: center; height: 36px; padding: 0 15px; display: inline-flex; align-items: center; gap: 14px; border: 1px solid var(--ink); border-radius: 10px; color: var(--paper); background: var(--ink); cursor: pointer; font: 650 12.5px/1 var(--font-display); white-space: nowrap; transition: background .15s, color .15s; }
.choose-button:not(:disabled):hover { background: var(--ink-2); }
.choose-button:disabled { cursor: not-allowed; }
/* 手机端压成单行投递条：首屏要让给文件架，而不是投递入口的排场 */
@media (max-width: 720px) {
  .dropzone { min-height: 52px; padding: 0 12px; grid-template-columns: 32px minmax(0, 1fr) auto; gap: 10px; border-radius: 12px; }
  .drop-index, .drop-copy > p, .upload-meta { display: none; }
  .upload-icon { width: 32px; height: 32px; border-radius: 50% 50% 8px; }
  .drop-copy h2 { font-size: 14px; }
  .choose-button { height: 32px; padding: 0 12px; gap: 6px; border-radius: 8px; font-size: 12px; }
}
</style>
