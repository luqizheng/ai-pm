<script setup lang="ts">
import Badge from "@/components/ui/badge/Badge.vue";
import type { DeliveryContextData } from "@/stores/reviewCenter";

/**
 * Delivery 业务上下文（V1.0 §5.3 + 用户裁决接 D5 闭环）：
 * 交付证据 / 验收标准核验 / 待处理审批提示。
 *
 * 纯展示件：数据来自 T-070 读模型（store 预载），文案由调用方翻译后经
 * props 传入（D8）。验收决策区由页面容器渲染（领域边界 D5）。
 * 有 pending 审批 ≠ 证据不可看：两种能力分开表达（V1.1 §5.3 文案修正）。
 */
interface Props {
  context: DeliveryContextData | null;
  loading: boolean;
  failed: boolean;
  contextTitle: string;
  loadingLabel: string;
  failedLabel: string;
  repositoryLabel: string;
  deliveryStateLabel: string;
  commitLabel: string;
  changedFilesLabel: string;
  verdictsLabel: string;
  verdictNotEvaluatedLabel: string;
  noEvidenceLabel: string;
  pendingNotice: string;
  noPendingNotice: string;
}

defineProps<Props>();

function commitText(row: DeliveryContextData["rows"][number]): string {
  return row.commit_sha ? row.commit_sha.slice(0, 10) : "—";
}
</script>

<template>
  <section class="delivery-ctx" aria-label="context">
    <h3 class="delivery-ctx__title">{{ contextTitle }}</h3>

    <p v-if="loading" class="delivery-ctx__note">{{ loadingLabel }}</p>
    <p v-else-if="failed || !context" class="delivery-ctx__note">
      {{ failedLabel }}
    </p>

    <template v-else>
      <div
        class="delivery-ctx__approval"
        :data-pending="Boolean(context.pendingApproval)"
        role="status"
      >
        {{ context.pendingApproval ? pendingNotice : noPendingNotice }}
      </div>

      <ul v-if="context.rows.length" class="delivery-ctx__rows">
        <li
          v-for="row in context.rows"
          :key="row.task_id + row.repository_id"
          class="delivery-ctx__row"
        >
          <div class="delivery-ctx__row-head">
            <span class="delivery-ctx__repo">{{ row.repository_name }}</span>
            <Badge variant="neutral" size="sm">{{ row.delivery_state }}</Badge>
          </div>
          <div class="delivery-ctx__row-meta">
            <span
              >{{ repositoryLabel }}: {{ row.repository_id.slice(0, 8) }}</span
            >
            <span>{{ commitLabel }}: {{ commitText(row) }}</span>
            <span v-if="row.changed_files !== null">
              {{ changedFilesLabel }}: {{ row.changed_files }}
            </span>
          </div>
        </li>
      </ul>
      <p v-else class="delivery-ctx__note">{{ noEvidenceLabel }}</p>

      <div v-if="context.verdicts && context.verdicts.length">
        <p class="delivery-ctx__verdicts-title">{{ verdictsLabel }}</p>
        <ul class="delivery-ctx__verdicts">
          <li v-for="verdict in context.verdicts" :key="verdict.criterion_id">
            <Badge
              :variant="
                verdict.verdict === 'passed'
                  ? 'success'
                  : verdict.verdict === 'failed'
                    ? 'danger'
                    : 'neutral'
              "
              size="sm"
            >
              {{ verdict.verdict ?? verdictNotEvaluatedLabel }}
            </Badge>
            <span>{{ verdict.text }}</span>
          </li>
        </ul>
      </div>
    </template>
  </section>
</template>

<style scoped>
.delivery-ctx__title {
  margin: 0 0 8px;
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 600;
}

.delivery-ctx__approval {
  margin-bottom: 12px;
  padding: 8px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  color: var(--text-secondary);
  font-size: 12px;
}

.delivery-ctx__approval[data-pending="true"] {
  color: var(--text-primary);
}

.delivery-ctx__rows {
  margin: 0 0 12px;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.delivery-ctx__row {
  padding: 10px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
}

.delivery-ctx__row-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.delivery-ctx__repo {
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 600;
}

.delivery-ctx__row-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 16px;
  margin-top: 6px;
  color: var(--text-secondary);
  font-size: 12px;
}

.delivery-ctx__verdicts-title {
  margin: 0 0 6px;
  color: var(--text-secondary);
  font-size: 12px;
}

.delivery-ctx__verdicts {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.delivery-ctx__verdicts li {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-secondary);
  font-size: 12px;
}

.delivery-ctx__note {
  margin: 0;
  color: var(--text-secondary);
  font-size: 13px;
}
</style>
