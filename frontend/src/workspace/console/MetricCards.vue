<script setup lang="ts">
import { metricCards } from "../fixtures";
</script>

<template>
  <section class="ws-metrics" aria-label="metrics">
    <div v-for="card in metricCards" :key="card.id" class="ws-card ws-metric">
      <div class="ws-metric-value" data-test="metric-value">
        {{ card.value }}
      </div>
      <div class="ws-metric-caption">{{ card.caption }}</div>
      <div class="ws-metric-sub">{{ card.sub }}</div>
    </div>
  </section>
</template>

<style scoped>
.ws-metrics {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.ws-metric {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px 16px;
}

.ws-metric-value {
  font-size: 24px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.ws-metric-caption {
  font-weight: 600;
}

.ws-metric-sub {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
