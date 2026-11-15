<script setup lang="ts">
import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Card from "@/components/ui/card/Card.vue";
import Descriptions from "@/components/ui/descriptions/Descriptions.vue";
import type { DeliveryContextData } from "@/stores/reviewCenter";

/**
 * Delivery 业务上下文（V1.0 §5.3 + 用户裁决接 D5 闭环）：
 * 交付证据 / 验收标准核验 / 待处理审批提示。
 *
 * 审批提示 = DS 组件 Alert；证据行 = DS 组件 Card + Descriptions。
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
      <Alert
        class="delivery-ctx__approval"
        :variant="context.pendingApproval ? 'warning' : 'info'"
        size="sm"
        role="status"
        :data-pending="Boolean(context.pendingApproval)"
      >
        {{ context.pendingApproval ? pendingNotice : noPendingNotice }}
      </Alert>

      <div v-if="context.rows.length" class="delivery-ctx__rows">
        <Card
          v-for="row in context.rows"
          :key="row.task_id + row.repository_id"
          class="delivery-ctx__row"
        >
          <template #header>
            <div class="delivery-ctx__row-head">
              <span class="delivery-ctx__repo">{{ row.repository_name }}</span>
              <Badge variant="neutral" size="sm">{{
                row.delivery_state
              }}</Badge>
            </div>
          </template>
          <Descriptions
            :items="[
              { label: repositoryLabel, value: row.repository_id.slice(0, 8) },
              { label: commitLabel, value: commitText(row) },
              {
                label: changedFilesLabel,
                value: row.changed_files === null ? null : row.changed_files
              }
            ]"
          />
        </Card>
      </div>
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
  margin: 0 0 var(--space-2);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
}

.delivery-ctx__approval {
  margin-bottom: var(--space-3);
}

.delivery-ctx__rows {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-bottom: var(--space-3);
}

/* 证据行是紧凑条目：清掉 Card 默认留白，键值区用紧凑间距 */
.delivery-ctx__row :deep(.ui-card__header) {
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-caption);
}

.delivery-ctx__row :deep(.ui-card__body) {
  padding: var(--space-2) var(--space-3) var(--space-3);
}

.delivery-ctx__row-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.delivery-ctx__repo {
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
}

.delivery-ctx__verdicts-title {
  margin: 0 0 var(--space-1);
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.delivery-ctx__verdicts {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.delivery-ctx__verdicts li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.delivery-ctx__note {
  margin: 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}
</style>
