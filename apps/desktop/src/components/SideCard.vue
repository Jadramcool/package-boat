<script setup lang="ts">
// 边栏卡统一外壳：右侧栏所有卡片共用同一标题条与内边距。
defineProps<{ title: string }>();
</script>

<template>
  <section class="side-card">
    <h2 class="side-card-title">{{ title }}</h2>
    <div class="side-card-body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.side-card {
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: 12px;
  background: var(--surface);
  box-shadow: 0 1px 2px rgb(31 39 28 / 4%);
}
.side-card-title {
  margin: 0;
  padding: 8px 13px;
  border-bottom: 1px solid var(--line);
  background: var(--dock-0);
  color: var(--muted);
  font: 650 11.5px/1 var(--font-label);
  letter-spacing: 0.11em;
}
.side-card-body {
  display: grid;
  gap: 10px;
  padding: 10px 12px;
}
</style>
