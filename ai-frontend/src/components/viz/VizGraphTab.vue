<script setup lang="ts">
/**
 * VizGraphTab —— 右栏 Graph 页签（V0-8）。
 *
 * 展示当前项目的图形产物槽位（列表 + 状态 + 缩略入口）。
 * 右栏宽度固定 288px（tokens.css `--ws-context-width`），因此只放列表与摘要，
 * **不放主画布**（主画布在独立视图与对话卡内）。
 *
 * 数据来自后端读接口（`GET /v1/pm/visualizations/projects/{id}/slots`），
 * 权限裁剪在服务端完成——本组件只渲染返回内容。
 */
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { listSlots, type VizSlotSummary } from "@/api/visualizations";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import Badge from "@/components/ui/badge/Badge.vue";

const { t } = useI18n();
const route = useRoute();
const scope = useScopeStore();

const slots = ref<VizSlotSummary[]>([]);
const loading = ref(false);
const failed = ref(false);

/** 项目 id：query 优先 → 回退 scope（排除「全部」哨兵）。 */
const projectId = computed(() => {
  const fromQuery = route.query.project_id;
  if (typeof fromQuery === "string" && fromQuery) return fromQuery;
  const current = scope.currentProjectId;
  // 哨兵值必须用 SCOPE_ALL 常量比对（值是 "__all__"）——写成字面量会把哨兵
  // 当成真实项目 id 发给后端。
  return current && current !== SCOPE_ALL ? current : "";
});

async function load(): Promise<void> {
  if (!projectId.value) return;
  loading.value = true;
  failed.value = false;
  try {
    const response = await listSlots(projectId.value);
    slots.value = response.items ?? [];
  } catch {
    failed.value = true;
    slots.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  void load();
});

function stateVariant(state: VizSlotSummary["state"]) {
  switch (state) {
    case "published":
      return "success" as const;
    case "stale":
      return "warning" as const;
    case "rejected":
      return "danger" as const;
    default:
      return "neutral" as const;
  }
}
</script>

<template>
  <div class="viz-graph-tab" :data-testid="'viz-graph-tab'">
    <p v-if="loading" class="viz-graph-tab__muted">
      {{ t("viz.tab.loading") }}
    </p>
    <p v-else-if="failed" class="viz-graph-tab__muted">
      {{ t("viz.tab.loadFailed") }}
    </p>
    <template v-else-if="slots.length === 0">
      <p class="viz-graph-tab__empty" :data-testid="'viz-graph-tab-empty'">
        {{ t("viz.tab.empty") }}
      </p>
      <p class="viz-graph-tab__muted">{{ t("viz.tab.emptyHint") }}</p>
    </template>
    <ul v-else class="viz-graph-tab__list">
      <li
        v-for="slot in slots"
        :key="slot.slot_key"
        class="viz-graph-tab__item"
        :data-testid="'viz-graph-slot'"
      >
        <span class="viz-graph-tab__name">{{ slot.slot_key }}</span>
        <span class="viz-graph-tab__meta">
          {{ t("viz.artifact.version", { version: slot.version_no }) }}
        </span>
        <Badge :variant="stateVariant(slot.state)" size="sm">
          {{ t(`viz.artifact.state.${slot.state}`) }}
        </Badge>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.viz-graph-tab {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.viz-graph-tab__list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.viz-graph-tab__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
}

.viz-graph-tab__name {
  flex: 1;
  min-width: 0;
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.viz-graph-tab__meta {
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.viz-graph-tab__empty,
.viz-graph-tab__muted {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.viz-graph-tab__empty {
  color: var(--text-secondary);
}
</style>
