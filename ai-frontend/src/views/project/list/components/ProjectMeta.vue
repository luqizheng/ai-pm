<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { FolderGit2 } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import type { ProjectEntry, ProjectLifecycle } from "../project-entry";

/**
 * 项目元信息（状态 + 仓库摘要 + 更新时间），列表行与网格卡片共用。
 *
 * 仓库摘要兼容三种形态（Projects List §6 + S01-03 裁决）：
 * 零仓库（暂无）/ 多仓库（计数 + 主仓库名）/ 无主仓库（未指定主仓库）；
 * 存在 unreachable 仓库时叠加警示徽标（「需要关注」的判定来源）。
 */
interface Props {
  entry: ProjectEntry;
  /** 布局语境：行内横排 / 卡片纵排 */
  layout?: "row" | "card";
}

const props = withDefaults(defineProps<Props>(), { layout: "row" });
const { t } = useI18n();

const statusLabel = computed(() => {
  const map: Record<ProjectLifecycle, string> = {
    active: t("pm.projects.statusActive"),
    placeholder: t("pm.projects.statusPlaceholder"),
    archived: t("pm.projects.statusArchived")
  };
  return map[props.entry.status];
});

const statusTone = computed<StatusIndicatorTone>(() => {
  const map: Record<ProjectLifecycle, StatusIndicatorTone> = {
    active: "success",
    placeholder: "warning",
    archived: "neutral"
  };
  return map[props.entry.status];
});

const repoLabel = computed(() => {
  const { count, primary_name } = props.entry.repositories;
  if (count === 0) return t("pm.projects.repoNone");
  if (primary_name) {
    return `${t("pm.projects.repoCount", { count })} · ${t("pm.projects.repoPrimary", { name: primary_name })}`;
  }
  return `${t("pm.projects.repoCount", { count })} · ${t("pm.projects.repoNoPrimary")}`;
});

const updatedAt = computed(() =>
  t("pm.projects.updatedAgo", { time: props.entry.updated_at })
);
</script>

<template>
  <div class="proj-meta" :class="`proj-meta--${props.layout}`">
    <span class="proj-meta__status">
      <StatusIndicator :status="statusTone" size="sm" />
      <span>{{ statusLabel }}</span>
    </span>
    <span class="proj-meta__repos">
      <FolderGit2 aria-hidden="true" />
      <span>{{ repoLabel }}</span>
    </span>
    <Badge v-if="entry.repositories.has_unreachable" variant="danger" size="sm">
      {{ t("pm.projects.repoUnreachable") }}
    </Badge>
    <span class="proj-meta__updated">{{ updatedAt }}</span>
  </div>
</template>

<style scoped>
.proj-meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.proj-meta--card {
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
}

.proj-meta__status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  white-space: nowrap;
}

.proj-meta__repos {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  min-width: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.proj-meta__repos svg {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

.proj-meta__repos span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.proj-meta__updated {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
</style>
