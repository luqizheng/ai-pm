<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import PageFrame from "@/components/shell/PageFrame.vue";
import Button from "@/components/ui/button/Button.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Table from "@/components/ui/table/Table.vue";
import TableBody from "@/components/ui/table/TableBody.vue";
import TableCell from "@/components/ui/table/TableCell.vue";
import TableHead from "@/components/ui/table/TableHead.vue";
import TableHeader from "@/components/ui/table/TableHeader.vue";
import TableRow from "@/components/ui/table/TableRow.vue";
import {
  fetchProduct,
  listProductTasks,
  setProductLifecycle,
  type ProductProfile,
  type ProductTask
} from "@/api/products";
import {
  formatDateTime,
  priorityGrade,
  aggregateStatusLabel,
  aggregateStatusTone
} from "@/views/project/tasks/tasks-contract";
import { describeError } from "@/lib/errors";

/**
 * PA-07 产品详情四页签（Figma 179:3603 E~E4「Billing API」页）：
 * Overview / Lifecycle / Tasks / History。
 *
 * Lifecycle = 四态指示器 + 门禁操作（suspend/resume/complete_resume/abandon，
 * 后端 Product Owner 独占 + noop 409）；Impact Summary 只陈述后端事实
 * （关联任务数与暂停期领取拒绝），「暂停范围外仍可执行」无 scope 模型不伪造。
 * History = 无产品审计端点，诚实空态。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const productId = computed(() => String(route.params.productId ?? ""));
const tab = computed(() => {
  const value = route.query.tab;
  return typeof value === "string" &&
    ["overview", "lifecycle", "tasks", "history"].includes(value)
    ? value
    : "overview";
});

const product = ref<ProductProfile | null>(null);
const tasks = ref<ProductTask[]>([]);
const loading = ref(false);
const loadError = ref<string | null>(null);
const actionError = ref<string | null>(null);
const acting = ref(false);

const TABS = ["overview", "lifecycle", "tasks", "history"] as const;

function switchTab(next: string) {
  void router.replace({ query: { ...route.query, tab: next } });
}

const restrictedCount = computed(
  () =>
    tasks.value.filter(
      task =>
        !["completed", "failed", "cancelled", "superseded"].includes(
          task.status
        )
    ).length
);

/** 各状态下允许的门禁操作（set_product_lifecycle action 映射） */
const lifecycleActions = computed(() => {
  const status = product.value?.status;
  if (status === "normal")
    return [
      {
        action: "suspend",
        label: t("pa07.products.lifecycleSuspend"),
        variant: "secondary" as const
      },
      {
        action: "abandon",
        label: t("pa07.products.lifecycleAbandon"),
        variant: "danger" as const
      }
    ];
  if (status === "suspended")
    return [
      {
        action: "resume",
        label: t("pa07.products.lifecycleResume"),
        variant: "primary" as const
      },
      {
        action: "abandon",
        label: t("pa07.products.lifecycleAbandon"),
        variant: "danger" as const
      }
    ];
  if (status === "resuming")
    return [
      {
        action: "complete_resume",
        label: t("pa07.products.lifecycleRestart"),
        variant: "primary" as const
      }
    ];
  return [];
});

function statusLabel(status: string): string {
  const key = `pa07.products.status${status.charAt(0).toUpperCase() + status.slice(1)}`;
  const label = t(key);
  return label === key ? status : label;
}

async function runLifecycle(action: string) {
  if (!product.value) return;
  acting.value = true;
  actionError.value = null;
  try {
    product.value = await setProductLifecycle(
      product.value.id,
      action as never
    );
  } catch (error) {
    actionError.value = describeError(error);
  } finally {
    acting.value = false;
  }
}

async function load() {
  if (!productId.value) return;
  loading.value = true;
  loadError.value = null;
  try {
    const [profile, taskRows] = await Promise.all([
      fetchProduct(productId.value),
      listProductTasks(productId.value)
    ]);
    product.value = profile;
    tasks.value = taskRows;
  } catch (error) {
    loadError.value = describeError(error);
  } finally {
    loading.value = false;
  }
}

onMounted(() => void load());
watch(productId, value => {
  if (value) void load();
});

function openTask(id: string) {
  void router.push(`/project/tasks/${id}`);
}
</script>

<template>
  <PageFrame
    :title="product?.name ?? t('pa07.products.detailTitle')"
    :description="
      product ? `${product.key} · ${statusLabel(product.status)}` : ''
    "
  >
    <p v-if="loadError" class="pd-error">{{ loadError }}</p>
    <p v-else-if="loading" class="pd-note">{{ t("common.loading") }}</p>
    <EmptyState
      v-else-if="!product"
      :title="t('pa07.dependencies.loadFailed')"
    />

    <template v-else>
      <div class="pd-head">
        <div class="pd-tabs" data-testid="product-tabs">
          <button
            v-for="item in TABS"
            :key="item"
            class="pd-tabs__tab"
            :class="{ 'is-active': tab === item }"
            :data-testid="`product-tab-${item}`"
            @click="switchTab(item)"
          >
            {{
              t(
                `pa07.products.tab${item.charAt(0).toUpperCase() + item.slice(1)}`
              )
            }}
          </button>
        </div>
        <Badge
          class="pd-status"
          :variant="
            product.status === 'normal' || product.status === 'resuming'
              ? 'success'
              : 'warning'
          "
          :outline="true"
          data-testid="product-status-badge"
        >
          {{ statusLabel(product.status) }}
        </Badge>
      </div>

      <!-- Overview -->
      <section
        v-if="tab === 'overview'"
        class="pd-section"
        data-testid="product-overview"
      >
        <dl class="pd-facts">
          <div>
            <dt>Key</dt>
            <dd>{{ product.key }}</dd>
          </div>
          <div>
            <dt>
              {{ t("pa07.products.taskCountLabel", { count: tasks.length }) }}
            </dt>
            <dd />
          </div>
        </dl>
        <p class="pd-copy">{{ product.description ?? "—" }}</p>
      </section>

      <!-- Lifecycle -->
      <section
        v-else-if="tab === 'lifecycle'"
        class="pd-section"
        data-testid="product-lifecycle"
      >
        <div class="pd-impact">
          <p class="pd-impact__title">
            {{
              t("pa07.products.impactBlockedTitle", { count: restrictedCount })
            }}
          </p>
          <p class="pd-impact__note">{{ t("pa07.products.impactNote") }}</p>
        </div>
        <div class="pd-restricted">
          <p class="pd-impact__title">
            {{ t("pa07.products.restrictedTitle") }}
          </p>
          <p class="pd-impact__note">{{ t("pa07.products.restrictedCopy") }}</p>
        </div>
        <div class="pd-actions" data-testid="product-lifecycle-actions">
          <Button
            v-for="item in lifecycleActions"
            :key="item.action"
            :variant="item.variant"
            size="md"
            :loading="acting"
            :data-test="`product-lifecycle-${item.action}`"
            @click="runLifecycle(item.action)"
          >
            {{ item.label }}
          </Button>
        </div>
        <p
          v-if="actionError"
          class="pd-error"
          data-test="product-lifecycle-error"
        >
          {{ actionError }}
        </p>
      </section>

      <!-- Tasks -->
      <section
        v-else-if="tab === 'tasks'"
        class="pd-section"
        data-testid="product-tasks"
      >
        <EmptyState
          v-if="tasks.length === 0"
          :title="t('pa07.products.tasksEmpty')"
        />
        <Table v-else data-test="product-tasks-table">
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("pa07.list.colTask") }}</TableHead>
              <TableHead>{{ t("pa07.list.colStatus") }}</TableHead>
              <TableHead>{{ t("pa07.list.colCondition") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="task in tasks"
              :key="task.id"
              data-test="product-task-row"
            >
              <TableCell>
                <span class="pd-task-line" @click="openTask(task.id)">
                  <span class="pd-task-code">{{ task.code }}</span>
                  <span class="pd-task-title">{{ task.title }}</span>
                  <span class="pd-task-meta">
                    {{ priorityGrade(task.priority) }}
                  </span>
                </span>
              </TableCell>
              <TableCell>
                <span
                  class="pd-task-status"
                  :class="`pd-task-status--${aggregateStatusTone(task.status)}`"
                >
                  {{ aggregateStatusLabel(task.status) }}
                </span>
              </TableCell>
              <TableCell
                >{{ task.deps_met }}/{{ task.deps_total }} met</TableCell
              >
            </TableRow>
          </TableBody>
        </Table>
      </section>

      <!-- History：无产品审计端点，诚实空态 -->
      <section v-else class="pd-section" data-testid="product-history">
        <EmptyState :title="t('pa07.products.historyEmpty')" />
      </section>
    </template>
  </PageFrame>
</template>

<style scoped>
.pd-error {
  margin: 0;
  color: var(--ws-color-danger-text);
}

.pd-note {
  margin: 0;
  color: var(--ws-text-secondary);
}

.pd-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.pd-tabs {
  display: flex;
  gap: var(--space-1);
}

.pd-tabs__tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  border: none;
  background: none;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-text-secondary);
  cursor: pointer;
}

.pd-tabs__tab.is-active {
  color: var(--ws-text-primary);
}

.pd-tabs__tab.is-active::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--ws-accent);
  content: "";
}

.pd-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.pd-facts {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.pd-facts > div {
  display: flex;
  gap: var(--space-3);
}

.pd-facts dt {
  width: 96px;
  flex-shrink: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.pd-facts dd {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-primary);
}

.pd-copy {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
}

.pd-impact,
.pd-restricted {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-card);
  background: var(--surface-raised);
}

.pd-impact__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.pd-impact__note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.pd-actions {
  display: flex;
  gap: var(--space-3);
}

.pd-task-line {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  cursor: pointer;
}

.pd-task-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.pd-task-title {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: 500;
  color: var(--ws-text-primary);
}

.pd-task-meta {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.pd-task-status {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.pd-task-status--success {
  color: var(--ws-color-success-text);
}

.pd-task-status--danger {
  color: var(--ws-color-danger-text);
}

.pd-task-status--info {
  color: var(--ws-color-info-text);
}

.pd-task-status--warning {
  color: var(--ws-color-warning-text);
}

.pd-task-status--muted,
.pd-task-status--neutral {
  color: var(--ws-text-secondary);
}
</style>
