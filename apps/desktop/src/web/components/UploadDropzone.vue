<script setup lang="ts">
import { ArrowUpRight, CloudUpload } from '@lucide/vue'
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
    @click="chooseFiles"
    @dragenter.prevent="handleDragEnter"
    @dragover.prevent
    @dragleave="handleDragLeave"
    @drop.prevent="handleDrop"
  >
    <input ref="fileInput" class="visually-hidden" type="file" multiple @change="handleChange">
    <div class="drop-icon" aria-hidden="true"><CloudUpload :size="17" :stroke-width="1.8" /></div>
    <div class="drop-copy">
      <h2 id="upload-title">投递文件到主机</h2>
      <p>{{ disabled ? '正在等待网络重新连接' : `拖到这里，或点击选择 · 单件上限 ${maxSizeLabel}` }}</p>
    </div>
    <button class="choose-button" type="button" :disabled="disabled" @click.stop="chooseFiles">
      选择文件 <ArrowUpRight :size="15" aria-hidden="true" />
    </button>
  </section>
</template>

<style scoped>
/* 单行主行动条：本页唯一酸绿实底；离线时降级为虚线中性 */
.dropzone {
  min-height: 58px;
  padding: 10px 16px;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 14px;
  border: 1px solid var(--ink);
  border-radius: 14px;
  background: var(--acid);
  cursor: pointer;
  transition: border-color .16s, transform .16s, box-shadow .16s, background .16s;
}
.dropzone.dragging {
  transform: translate(-3px, -3px);
  box-shadow: 6px 6px 0 var(--ink);
  background: var(--acid-hi);
}
.dropzone.disabled {
  border: 1.5px dashed var(--line-strong);
  background: var(--surface);
  cursor: not-allowed;
  opacity: .72;
}
.drop-icon {
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  background: var(--ink);
  color: var(--acid);
  flex: none;
}
.dropzone.disabled .drop-icon {
  background: var(--line-strong);
  color: var(--muted);
}
.drop-copy { min-width: 0; }
.drop-copy h2 {
  margin: 0;
  color: var(--ink);
  font: 700 15px/1.25 var(--font-display);
}
.drop-copy > p {
  margin: 4px 0 0;
  color: rgb(31 39 28 / 72%);
  font-size: 12px;
  line-height: 1.35;
}
.dropzone.disabled .drop-copy h2,
.dropzone.disabled .drop-copy > p { color: var(--muted); }
.choose-button {
  flex: none;
  height: 36px;
  padding: 0 14px;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--ink);
  border-radius: 10px;
  color: var(--paper);
  background: var(--ink);
  cursor: pointer;
  font: 650 12.5px/1 var(--font-label);
  letter-spacing: .06em;
  white-space: nowrap;
}
.choose-button:not(:disabled):hover { background: var(--ink-2); }
.choose-button:disabled { cursor: not-allowed; opacity: .55; }
@media (max-width: 560px) {
  .choose-button { display: none; }
  .drop-copy > p { font-size: 10.5px; }
}
</style>
