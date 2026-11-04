<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/ProjectSwitcher/Menu" node=33:87796 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { Check, ChevronRight, FileText, SearchX } from "lucide-vue-next";

import { SearchInput } from "@/components/ui/search-input";
import { ListItem } from "@/components/ui/list-item";
import { EmptyState } from "@/components/ui/empty-state";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";

import {
  PROJECT_SWITCHER_DEFAULT_TEXT,
  PROJECT_SWITCHER_GROUPS,
  type ProjectSwitcherProject,
  type ProjectSwitcherStatus,
  type ProjectSwitcherText
} from "./project-switcher";

/**
 * ProjectSwitcherMenu —— Figma「06 — Project Context Switcher」的
 * Global/ProjectSwitcher/Menu 面板（33:87796），Dark/Light 随主题 token。
 *
 * 反馈态（status）：
 *   - ready：标题 + 搜索 + FAVORITES/RECENT 分区行 + Create + 说明
 *   - loading：标题 + 双骨架行（绝不渲染旧列表，Figma 行为注）
 *   - empty：无可用项目空态（Create 仅在 canCreate 时可点）
 *   - error：danger Alert（当前项目保留）+ Retry
 * 搜索无结果（query 非空且零命中）= NoResults 空态 + Clear query。
 *
 * 文案经 texts 注入（默认 Figma 原文，应用层可绑定 i18n）。
 */
interface Props {
  projects: ProjectSwitcherProject[];
  currentId?: string | null;
  status?: ProjectSwitcherStatus;
  query?: string;
  /** 是否允许创建项目（false 时 Create 禁用，不泄露隐藏项目） */
  canCreate?: boolean;
  texts?: Partial<ProjectSwitcherText>;
}

const props = withDefaults(defineProps<Props>(), {
  currentId: null,
  status: "ready",
  query: "",
  canCreate: true,
  texts: () => ({})
});

const emit = defineEmits<{
  select: [id: string];
  retry: [];
  create: [];
  "clear-query": [];
  "update:query": [value: string];
}>();

const t = computed<ProjectSwitcherText>(() => ({
  ...PROJECT_SWITCHER_DEFAULT_TEXT,
  ...props.texts
}));

const sections = computed(() => {
  const q = props.query.trim().toLowerCase();
  return PROJECT_SWITCHER_GROUPS.map(group => ({
    group,
    items: props.projects.filter(
      p => p.group === group && (!q || p.name.toLowerCase().includes(q))
    )
  })).filter(section => section.items.length > 0);
});

const isNoResults = computed(
  () =>
    props.status === "ready" &&
    props.query.trim().length > 0 &&
    sections.value.length === 0
);

const noResultsDescription = computed(
  () => `No available project matches “${props.query.trim()}”.`
);
</script>

<template>
  <div class="ps-menu">
    <!-- ── 就绪：标题 + 搜索 + 分区列表 + Create + 说明 ─────────────── -->
    <template v-if="props.status === 'ready'">
      <p class="ps-menu__title">{{ t.title }}</p>

      <SearchInput
        :model-value="props.query"
        size="md"
        :placeholder="t.searchPlaceholder"
        shortcut=""
        :aria-label="t.searchPlaceholder"
        @update:model-value="emit('update:query', $event)"
      />

      <section
        v-for="section in sections"
        :key="section.group"
        class="ps-menu__section"
      >
        <p class="ps-menu__group">{{ t.groupLabels[section.group] }}</p>
        <ListItem
          v-for="project in section.items"
          :key="project.id"
          class="ps-menu__row"
          :class="{ 'is-current': project.id === props.currentId }"
          :title="project.name"
          :icon="FileText"
          :selected="project.id === props.currentId"
          @click="emit('select', project.id)"
        >
          <template #trailing>
            <template v-if="project.id === props.currentId">
              <span class="ps-menu__badge">{{ t.currentBadge }}</span>
              <Check class="ps-menu__row-check" :size="16" aria-hidden="true" />
            </template>
            <template v-else>
              <span v-if="project.meta" class="ps-menu__meta">{{
                project.meta
              }}</span>
              <ChevronRight
                class="ps-menu__row-chevron"
                :size="16"
                aria-hidden="true"
              />
            </template>
          </template>
        </ListItem>
      </section>

      <!-- 搜索无结果（Figma Search / No results：Clear query） -->
      <EmptyState
        v-if="isNoResults"
        class="ps-menu__feedback"
        :icon="SearchX"
        :title="t.noResultsTitle"
        :description="noResultsDescription"
        :action-label="t.clearLabel"
        @action="emit('clear-query')"
      />

      <Button
        class="ps-menu__create"
        variant="primary"
        size="sm"
        :disabled="!props.canCreate"
        @click="emit('create')"
      >
        {{ t.createLabel }}
      </Button>

      <p class="ps-menu__footer">
        {{ props.canCreate ? t.footerNote : t.createUnavailableNote }}
      </p>
    </template>

    <!-- ── 加载：标题 + 双骨架行（Figma Menu / Loading） ────────────── -->
    <div
      v-else-if="props.status === 'loading'"
      class="ps-menu__loading"
      role="status"
    >
      <p class="ps-menu__loading-label">{{ t.title }}</p>
      <div v-for="i in 2" :key="i" class="ps-menu__skeleton-row">
        <Skeleton
          class="ps-menu__skeleton-avatar"
          width="var(--size-control-md)"
          height="var(--size-control-md)"
        />
        <div class="ps-menu__skeleton-lines">
          <Skeleton tone="strong" height="var(--space-3)" />
          <Skeleton tone="base" width="70%" height="10px" />
        </div>
      </div>
    </div>

    <!-- ── 无可用项目（Figma Menu / Empty） ────────────────────────── -->
    <EmptyState
      v-else-if="props.status === 'empty'"
      class="ps-menu__feedback"
      :icon="SearchX"
      :title="t.emptyTitle"
      :description="t.emptyDescription"
      :action-label="props.canCreate ? t.createLabel : ''"
      @action="emit('create')"
    />

    <!-- ── 加载失败（Figma Menu / Error：当前项目保留 + Retry） ────── -->
    <template v-else-if="props.status === 'error'">
      <Alert variant="danger" size="md" :title="t.errorTitle">
        {{ t.errorDescription }}
      </Alert>
      <Button
        class="ps-menu__retry"
        variant="secondary"
        size="sm"
        @click="emit('retry')"
      >
        {{ t.retryLabel }}
      </Button>
    </template>
  </div>
</template>

<style scoped>
/* 面板（Figma：radius 8 / padding 16 / gap 12 / surface 底 + 1px 边框） */
.ps-menu {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  width: var(--size-project-menu);
  max-width: 100%;
  padding: var(--space-4);
  background-color: var(--surface-base);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}

.ps-menu__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

/* 分区头：mono 大写次文字（FAVORITES / RECENT） */
.ps-menu__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ps-menu__group {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: var(--tracking-caption);
}

/* 项目行（Figma：h36 / padding 0 12 / 1px 边框，selected 描边 accent）。
   边框为本板实例覆盖口径（DS/ListItem 母版无边框），故落本层不动原语。 */
.ps-menu__row {
  height: var(--ws-row-height);
  padding: 0 var(--space-3);
  background-color: transparent;
  border: var(--border-1) solid var(--border-default);
}

.ps-menu__row.is-current {
  border-color: var(--accent);
}

.ps-menu__badge,
.ps-menu__meta {
  flex: none;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  white-space: nowrap;
}

.ps-menu__row-check {
  flex: none;
  color: var(--accent);
}

.ps-menu__row-chevron {
  flex: none;
  color: var(--text-secondary);
}

/* 反馈卡（Empty / NoResults：surface 底 + 1px 边框 + padding 24 居中） */
.ps-menu__feedback {
  padding: var(--space-6);
  background-color: var(--surface-base);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}

/* 加载态（Figma：标签 + 头像 36 + 双占位线，gap 12） */
.ps-menu__loading {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.ps-menu__loading-label {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.ps-menu__skeleton-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: var(--ws-row-height);
}

.ps-menu__skeleton-avatar {
  flex: none;
  border-radius: var(--radius-sm);
}

.ps-menu__skeleton-lines {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-1);
}

/* Create / Retry 行（Figma：sm 按钮 hug 宽，不随面板拉伸） */
.ps-menu__create,
.ps-menu__retry {
  align-self: flex-start;
}

.ps-menu__footer {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
}
</style>
