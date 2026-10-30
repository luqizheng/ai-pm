<script setup lang="ts">
defineProps<{ title: string; subtitle?: string }>();
</script>

<template>
  <header class="ws-page-header">
    <h1 class="ws-page-title">{{ title }}</h1>
    <p v-if="subtitle" class="ws-page-subtitle">{{ subtitle }}</p>
  </header>
</template>

<style scoped>
.ws-page-header {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ws-page-title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: 600;
}

.ws-page-subtitle {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
