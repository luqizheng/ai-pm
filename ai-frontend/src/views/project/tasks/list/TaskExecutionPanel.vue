<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { formatDateTime } from "../tasks-contract";
import type { RelatedProduct } from "./task-list-adapter";

/**
 * Execution context 右栏（PA-07 179:3603「Contextual execution panel」）。
 *
 * 三段固定内容 + RELATED PRODUCTS 列表：
 *   1. Scheduling is calculated —— 调度语义一句话；
 *   2. RELATED PRODUCTS —— 项目关联产品 + 四态门禁状态（listProjectProducts）；
 *   3. CONTEXT, NOT A BYPASS —— 无 Force Start / Ignore Dependency 动作的产品
 *      裁决文案（后端 claim 事务内拦截是执法点，本栏只陈述事实）。
 * 数据不可得（未选项目/接口失败）时诚实空态，不伪造产品列表。
 */
interface Props {
  products: RelatedProduct[];
  updatedAt: string | null;
  loading?: boolean;
}

const props = defineProps<Props>();

const { t } = useI18n();

const PRODUCT_STATUS_LABELS: Record<string, string> = {
  normal: "Normal · Execution permitted",
  resuming: "Resuming · Execution permitted",
  suspended: "Suspended · Development restricted",
  archived: "Archived · Development restricted"
};

function productStatus(status: string): string {
  return PRODUCT_STATUS_LABELS[status] ?? status;
}

const updatedLabel = computed(() =>
  props.updatedAt ? formatDateTime(props.updatedAt) : null
);
</script>

<template>
  <aside class="exec-panel" data-testid="execution-context-panel">
    <p class="exec-panel__title">{{ t("pa07.panel.title") }}</p>
    <div class="exec-panel__rule" />

    <p class="exec-panel__caption">{{ t("pa07.panel.scopeCaption") }}</p>
    <p class="exec-panel__heading">{{ t("pa07.panel.schedulingTitle") }}</p>
    <p class="exec-panel__copy">{{ t("pa07.panel.schedulingCopy") }}</p>

    <div class="exec-panel__rule" />

    <p class="exec-panel__caption">{{ t("pa07.panel.relatedCaption") }}</p>
    <p
      v-if="props.loading"
      class="exec-panel__copy"
      data-testid="panel-loading"
    >
      {{ t("common.loading") }}
    </p>
    <p
      v-else-if="props.products.length === 0"
      class="exec-panel__copy"
      data-testid="panel-products-empty"
    >
      {{ t("pa07.panel.relatedEmpty") }}
    </p>
    <ul v-else class="exec-panel__products" data-testid="panel-products">
      <li
        v-for="product in props.products"
        :key="product.id"
        class="exec-panel__product"
      >
        <span class="exec-panel__product-name">{{ product.name }}</span>
        <span
          class="exec-panel__product-status"
          :class="{
            'is-restricted':
              product.status === 'suspended' || product.status === 'archived'
          }"
        >
          {{ productStatus(product.status) }}
        </span>
      </li>
    </ul>

    <div class="exec-panel__rule" />

    <p class="exec-panel__caption">{{ t("pa07.panel.bypassCaption") }}</p>
    <p class="exec-panel__copy">{{ t("pa07.panel.bypassCopy") }}</p>

    <p
      v-if="updatedLabel"
      class="exec-panel__updated"
      data-testid="panel-updated"
    >
      {{ t("pa07.panel.updated", { time: updatedLabel }) }}
    </p>
  </aside>
</template>

<style scoped>
/* 288~304px 右栏：surface-raised 底 + 分段 rule，同设计稿 Contextual execution panel */
.exec-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5) var(--space-5);
  border-left: var(--border-1) solid var(--ws-border);
  background: var(--surface-raised);
}

.exec-panel__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.exec-panel__caption {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: 500;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--ws-text-secondary);
}

.exec-panel__heading {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.exec-panel__copy {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: 1.45;
  color: var(--ws-text-secondary);
}

.exec-panel__rule {
  height: var(--border-1);
  background: var(--ws-border);
}

.exec-panel__products {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.exec-panel__product {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.exec-panel__product-name {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
}

.exec-panel__product-status {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.exec-panel__product-status.is-restricted {
  color: var(--ws-color-danger-text);
}

.exec-panel__updated {
  margin: auto 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-disabled);
}
</style>
