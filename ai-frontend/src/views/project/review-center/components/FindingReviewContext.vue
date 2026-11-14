<script setup lang="ts">
import Badge from "@/components/ui/badge/Badge.vue";
import type { DiscoveryFinding } from "@/api/discovery";

/**
 * Finding 业务上下文（V1.0 §5.1）：发现内容 / 类型 / 置信度 / 证据。
 *
 * 纯展示件：数据来自 discovery session 读端点（store 预载），
 * 文案由调用方翻译后经 props 传入（D8）。原始来源仅展示实际存在字段。
 */
interface Props {
  finding: DiscoveryFinding | null;
  loading: boolean;
  failed: boolean;
  contextTitle: string;
  loadingLabel: string;
  failedLabel: string;
  kindLabel: string;
  confidenceLabel: string;
  evidenceLabel: string;
  noEvidenceLabel: string;
}

defineProps<Props>();

function confidenceText(confidence: number | null): string {
  return confidence === null ? "—" : `${Math.round(confidence * 100)}%`;
}
</script>

<template>
  <section class="finding-ctx" aria-label="context">
    <h3 class="finding-ctx__title">{{ contextTitle }}</h3>

    <p v-if="loading" class="finding-ctx__note">{{ loadingLabel }}</p>
    <p v-else-if="failed || !finding" class="finding-ctx__note">
      {{ failedLabel }}
    </p>

    <template v-else>
      <p class="finding-ctx__content">{{ finding.content }}</p>
      <dl class="finding-ctx__facts">
        <div class="finding-ctx__fact">
          <dt>{{ kindLabel }}</dt>
          <dd>{{ finding.kind }}</dd>
        </div>
        <div class="finding-ctx__fact">
          <dt>{{ confidenceLabel }}</dt>
          <dd>{{ confidenceText(finding.confidence) }}</dd>
        </div>
      </dl>
      <div v-if="finding.evidence.length" class="finding-ctx__evidence">
        <p class="finding-ctx__evidence-title">{{ evidenceLabel }}</p>
        <ul>
          <li v-for="(item, index) in finding.evidence" :key="index">
            <Badge v-if="item.file" variant="neutral" size="sm">
              {{ item.file }}
            </Badge>
            <span v-if="item.description">{{ item.description }}</span>
          </li>
        </ul>
      </div>
      <p v-else class="finding-ctx__note">{{ noEvidenceLabel }}</p>
    </template>
  </section>
</template>

<style scoped>
.finding-ctx__title {
  margin: 0 0 8px;
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 600;
}

.finding-ctx__content {
  margin: 0 0 12px;
  color: var(--text-primary);
  font-size: 14px;
  line-height: 1.6;
  white-space: pre-wrap;
}

.finding-ctx__facts {
  display: flex;
  gap: 24px;
  margin: 0 0 12px;
}

.finding-ctx__fact dt {
  color: var(--text-secondary);
  font-size: 12px;
}

.finding-ctx__fact dd {
  margin: 2px 0 0;
  color: var(--text-primary);
  font-size: 13px;
}

.finding-ctx__evidence-title {
  margin: 0 0 6px;
  color: var(--text-secondary);
  font-size: 12px;
}

.finding-ctx__evidence ul {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.finding-ctx__evidence li {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-secondary);
  font-size: 12px;
}

.finding-ctx__note {
  margin: 0;
  color: var(--text-secondary);
  font-size: 13px;
}
</style>
