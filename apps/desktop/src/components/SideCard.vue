<script setup lang="ts">
// 边栏卡统一外壳：右侧栏所有卡片共用同一标题条与内边距，
// 避免「连接设置」与「接收设置」来源不同而露出拼接感。
// `flush` 用于数据网格类卡片（如清单概览），让内容贴齐卡片边缘。
defineProps<{ title: string; flush?: boolean }>();
</script>

<template>
  <section class="side-card">
    <h2 class="side-card-title">{{ title }}</h2>
    <div class="side-card-body" :class="{ flush }">
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
/* 标题条上下 8px、body 上下 10px：扫码卡新增「需要配对码」开关后，
   两卡合计必须回到 1280×764 的 main 可视高度（588px）以内。 */
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
.side-card-body.flush {
  padding: 0;
  gap: 0;
}
</style>

