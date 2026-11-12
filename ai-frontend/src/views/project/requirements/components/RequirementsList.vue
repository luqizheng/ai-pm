<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { LayoutList, Rows3 } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";

import { SearchForm } from "@/components/ui/search-form";
import SearchInput from "@/components/ui/search-input/SearchInput.vue";
import Select from "@/components/ui/select/Select.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import type {
  RequirementEntry,
  RequirementPriority,
  RequirementType,
  RequirementUiState
} from "../requirement-entry";

/**
 * 需求列表（S02-UI-01 §6.1）。
 *
 * 工具栏：搜索 + 状态/类型/优先级过滤 + 排序 + 列表/紧凑视图。
 * 状态过滤受控（摘要条联动）；其余控件组件内自管理。
 * 行展示：编码 / 标题+短描述 / 类型 / 优先级 / 生命周期状态 / 来源指示 / 更新时间。
 */
interface Props {
  entries: RequirementEntry[];
  selectedId: string;
  /** 状态过滤（摘要条受控；空 = 全部） */
  statusFilter: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "select", id: string): void;
  (e: "update:statusFilter", value: string): void;
}>();

const { t } = useI18n();

const search = ref("");
const typeFilter = ref("all");
const priorityFilter = ref("all");
const sortKey = ref("updated_desc");
const viewMode = ref<"list" | "compact">("list");

const statusOptions = computed(() => [
  { value: "all", label: t("pm.requirements.filterAllStatus") },
  { value: "draft", label: t("pm.requirements.statusDraft") },
  {
    value: "pending_confirmation",
    label: t("pm.requirements.statusPending")
  },
  { value: "confirmed", label: t("pm.requirements.statusConfirmed") },
  { value: "conflict", label: t("pm.requirements.statusConflict") }
]);

const typeOptions = computed(() => [
  { value: "all", label: t("pm.requirements.filterAllTypes") },
  { value: "functional", label: t("pm.requirements.typeFunctional") },
  {
    value: "non_functional",
    label: t("pm.requirements.typeNonFunctional")
  }
]);

const priorityOptions = computed(() => [
  { value: "all", label: t("pm.requirements.filterAllPriorities") },
  { value: "high", label: t("pm.requirements.priorityHigh") },
  { value: "medium", label: t("pm.requirements.priorityMedium") },
  { value: "low", label: t("pm.requirements.priorityLow") }
]);

const sortOptions = computed(() => [
  { value: "updated_desc", label: t("pm.requirements.sortUpdatedDesc") },
  { value: "updated_asc", label: t("pm.requirements.sortUpdatedAsc") },
  {
    value: "priority",
    label: t("pm.requirements.sortPriority")
  }
]);

const priorityWeight: Record<RequirementPriority, number> = {
  high: 0,
  medium: 1,
  low: 2
};

const visibleEntries = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  const filtered = props.entries.filter(entry => {
    if (props.statusFilter !== "all" && entry.status !== props.statusFilter)
      return false;
    if (typeFilter.value !== "all" && entry.type !== typeFilter.value)
      return false;
    if (
      priorityFilter.value !== "all" &&
      entry.priority !== priorityFilter.value
    )
      return false;
    if (keyword) {
      const haystack =
        `${entry.code} ${entry.title} ${entry.description}`.toLowerCase();
      if (!haystack.includes(keyword)) return false;
    }
    return true;
  });

  return [...filtered].sort((a, b) => {
    if (sortKey.value === "updated_asc") {
      return a.updated_at < b.updated_at ? -1 : 1;
    }
    if (sortKey.value === "priority") {
      return priorityWeight[a.priority] - priorityWeight[b.priority];
    }
    return a.updated_at > b.updated_at ? -1 : 1;
  });
});

const typeLabel = (type: RequirementType) =>
  type === "functional"
    ? t("pm.requirements.typeFunctional")
    : t("pm.requirements.typeNonFunctional");

const priorityLabel = (priority: RequirementPriority) =>
  priority === "high"
    ? t("pm.requirements.priorityHigh")
    : priority === "medium"
      ? t("pm.requirements.priorityMedium")
      : t("pm.requirements.priorityLow");

const statusLabel = (status: RequirementUiState) =>
  ({
    draft: t("pm.requirements.statusDraft"),
    pending_confirmation: t("pm.requirements.statusPending"),
    confirmed: t("pm.requirements.statusConfirmed"),
    conflict: t("pm.requirements.statusConflict")
  })[status];

const statusTone = (status: RequirementUiState): StatusIndicatorTone =>
  (
    ({
      draft: "neutral",
      pending_confirmation: "warning",
      confirmed: "success",
      conflict: "danger"
    }) as const
  )[status];

const isCompact = computed(() => viewMode.value === "compact");

watch(
  () => props.statusFilter,
  value => {
    if (
      value &&
      !visibleEntries.value.some(entry => entry.id === props.selectedId)
    ) {
      emit("select", visibleEntries.value[0]?.id ?? "");
    }
  }
);
</script>

<template>
  <section class="req-list" :aria-label="t('pm.requirements.listAria')">
    <SearchForm class="req-list__toolbar">
      <template #search>
        <SearchInput
          v-model="search"
          :placeholder="t('pm.requirements.searchPlaceholder')"
          size="sm"
        />
      </template>
      <Select
        :model-value="props.statusFilter"
        :options="statusOptions"
        size="sm"
        :aria-label="t('pm.requirements.filterStatus')"
        @update:model-value="emit('update:statusFilter', $event)"
      />
      <Select
        v-model="typeFilter"
        :options="typeOptions"
        size="sm"
        :aria-label="t('pm.requirements.filterType')"
      />
      <Select
        v-model="priorityFilter"
        :options="priorityOptions"
        size="sm"
        :aria-label="t('pm.requirements.filterPriority')"
      />
      <Select
        v-model="sortKey"
        :options="sortOptions"
        size="sm"
        :aria-label="t('pm.requirements.sortLabel')"
      />
      <template #append>
        <div
          class="req-list__view-toggle"
          role="group"
          :aria-label="t('pm.requirements.viewToggle')"
        >
          <button
            type="button"
            class="req-list__view-btn"
            :class="{ 'req-list__view-btn--active': !isCompact }"
            :aria-pressed="!isCompact"
            :aria-label="t('pm.requirements.viewList')"
            @click="viewMode = 'list'"
          >
            <LayoutList aria-hidden="true" />
          </button>
          <button
            type="button"
            class="req-list__view-btn"
            :class="{ 'req-list__view-btn--active': isCompact }"
            :aria-pressed="isCompact"
            :aria-label="t('pm.requirements.viewCompact')"
            @click="viewMode = 'compact'"
          >
            <Rows3 aria-hidden="true" />
          </button>
        </div>
      </template>
    </SearchForm>

    <EmptyState
      v-if="visibleEntries.length === 0"
      :title="t('pm.requirements.listEmptyTitle')"
      :description="t('pm.requirements.listEmptyDesc')"
    />
    <ul v-else class="req-list__rows">
      <li v-for="entry in visibleEntries" :key="entry.id">
        <button
          type="button"
          class="req-row"
          :class="[
            `req-row--${viewMode}`,
            { 'req-row--selected': entry.id === props.selectedId }
          ]"
          :aria-current="entry.id === props.selectedId ? 'true' : undefined"
          @click="emit('select', entry.id)"
        >
          <span class="req-row__code">{{ entry.code }}</span>
          <span v-if="!isCompact" class="req-row__main">
            <span class="req-row__title">{{ entry.title }}</span>
            <span class="req-row__desc">{{ entry.description }}</span>
          </span>
          <span v-else class="req-row__main req-row__main--compact">
            <span class="req-row__title">{{ entry.title }}</span>
          </span>
          <Badge v-if="!isCompact" variant="neutral" size="sm">
            {{ typeLabel(entry.type) }}
          </Badge>
          <Badge
            v-if="!isCompact"
            :variant="entry.priority === 'high' ? 'danger' : 'neutral'"
            size="sm"
          >
            {{ priorityLabel(entry.priority) }}
          </Badge>
          <span class="req-row__status">
            <StatusIndicator :status="statusTone(entry.status)" size="sm" />
            <span>{{ statusLabel(entry.status) }}</span>
          </span>
          <span v-if="!isCompact" class="req-row__meta">
            <span class="req-row__source">{{ entry.source_label }}</span>
            <span class="req-row__updated">{{ entry.updated_at }}</span>
          </span>
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.req-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.req-list__view-toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}

.req-list__view-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard);
}

.req-list__view-btn svg {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
}

.req-list__view-btn:hover {
  background: var(--ws-hover-veil);
  color: var(--text-primary);
}

.req-list__view-btn--active {
  background: var(--ws-color-accent-surface);
  border-color: var(--accent);
  color: var(--accent);
}

.req-list__view-btn:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.req-list__rows {
  margin: 0;
  padding: 0;
  list-style: none;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  overflow: hidden;
}

.req-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  min-height: var(--size-row);
  padding: var(--space-2) var(--space-4);
  border: 0;
  border-bottom: var(--hairline) solid var(--border-default);
  background: transparent;
  font-family: var(--font-ui);
  text-align: left;
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-standard);
}

.req-list__rows li:last-child .req-row {
  border-bottom: 0;
}

.req-row:hover {
  background: var(--ws-hover-veil);
}

.req-row:focus-visible {
  outline: none;
  box-shadow: inset var(--ws-focus-ring);
}

.req-row--selected {
  background: var(--ws-color-accent-surface);
}

.req-row--compact {
  min-height: var(--size-nav-item);
  padding-top: var(--space-1);
  padding-bottom: var(--space-1);
}

.req-row__code {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.req-row__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1 1 0;
  min-width: 0;
}

.req-row__title {
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.req-row__desc {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.req-row__main--compact {
  flex-direction: row;
}

.req-row__status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  white-space: nowrap;
}

.req-row__meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: var(--space-1);
  flex: none;
  min-width: 120px;
}

.req-row__source {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-row__updated {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
}
</style>
