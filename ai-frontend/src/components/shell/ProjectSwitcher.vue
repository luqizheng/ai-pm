<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger
} from "reka-ui";
import { Box, ChevronDown, LoaderCircle, Search } from "lucide-vue-next";

import type { ProjectOption } from "@/stores/shell";

/**
 * 项目切换器（Figma「04 — Global Navigation」Project Context Header 三态 +
 * 「07 — Project Switching Navigation Transition」的 Project switcher menu）。
 *
 * 三态（组件库 7:16866~7:16898）：
 *   - Selected：已选项目，透明底
 *   - Open：菜单展开，底色 elevated
 *   - Empty：未选项目，标题为「选择项目」
 *
 * 菜单（7:16466）：
 *   - 搜索行（可选，h36 / canvas 底 / 描边）
 *   - 项目条目（当前项为 Active：accent 着色面 + accent 图标 + 左侧指示条）
 *   - 切换中状态行（accent 转圈图标 + 「正在打开 X…」）
 *
 * 两个变体：
 *   - header：左侧导航的项目上下文头（h40 行）
 *   - chip：顶栏项目芯片（h32 行）
 *
 * 切换过程遵循 Figma「07」：进入切换态后**不显示旧项目数据**，
 * 由 store 的 switching 状态驱动（见 src/stores/shell.ts）。
 */

interface Props {
  /** 可选项目 */
  projects: ProjectOption[];
  /** 当前项目 id（空 = 未选择） */
  currentId?: string | null;
  /** 变体：左侧导航头（40px 行）/ 顶栏芯片（32px 行） */
  variant?: "header" | "chip";
  /** 正在切换到的项目 id（非空 = 切换中） */
  switchingToId?: string | null;
  /** 菜单是否含搜索行 */
  searchable?: boolean;
  /** 浮层相对触发器的位置 */
  side?: "top" | "right" | "bottom" | "left";
}

const props = withDefaults(defineProps<Props>(), {
  currentId: null,
  variant: "header",
  switchingToId: null,
  searchable: false,
  side: "bottom"
});

const emit = defineEmits<{ select: [id: string] }>();

const { t } = useI18n();

const open = ref(false);
const query = ref("");

const current = computed(
  () => props.projects.find(p => p.id === props.currentId) ?? null
);

const switchingTo = computed(
  () => props.projects.find(p => p.id === props.switchingToId) ?? null
);

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.projects;
  return props.projects.filter(p => p.name.toLowerCase().includes(q));
});

/** 触发器标题：未选项目时为「选择项目」，切换中为切换目标名 */
const title = computed(() => {
  if (props.switchingToId) {
    return switchingTo.value?.name ?? t("shell.app.project.switching");
  }
  return current.value?.name ?? t("shell.app.project.empty");
});

function onSelect(id: string) {
  open.value = false;
  query.value = "";
  emit("select", id);
}
</script>

<template>
  <PopoverRoot v-model:open="open">
    <!-- asChild：触发器即下面的真实 button（不额外包一层交互元素） -->
    <PopoverTrigger as-child>
      <button
        type="button"
        class="shell-project-switcher"
        :class="[
          `is-${props.variant}`,
          { 'is-open': open, 'is-empty': !current && !props.switchingToId }
        ]"
      >
        <Box
          :size="props.variant === 'chip' ? 14 : 16"
          class="shell-project-switcher__icon"
          aria-hidden="true"
        />
        <span class="shell-project-switcher__name">{{ title }}</span>
        <ChevronDown
          :size="12"
          class="shell-project-switcher__chevron"
          aria-hidden="true"
        />
      </button>
    </PopoverTrigger>

    <PopoverPortal>
      <PopoverContent
        class="shell-project-menu"
        :side="props.side"
        align="start"
        :side-offset="4"
      >
        <!-- 搜索行（Figma：Find a project…） -->
        <div v-if="props.searchable" class="shell-project-menu__search">
          <Search :size="14" aria-hidden="true" />
          <input
            v-model="query"
            type="text"
            class="shell-project-menu__search-input"
            :placeholder="t('shell.app.project.searchPlaceholder')"
            :aria-label="t('shell.app.project.searchPlaceholder')"
          />
        </div>

        <!-- 项目条目 -->
        <button
          v-for="project in filtered"
          :key="project.id"
          type="button"
          class="shell-project-menu__item"
          :class="{ 'is-active': project.id === props.currentId }"
          role="menuitem"
          @click="onSelect(project.id)"
        >
          <span class="shell-project-menu__active-bar" aria-hidden="true" />
          <Box :size="16" aria-hidden="true" />
          <span class="shell-project-menu__item-name">{{ project.name }}</span>
        </button>

        <!-- 切换中状态（Figma：Opening Research Sandbox…） -->
        <p v-if="props.switchingToId" class="shell-project-menu__status">
          <LoaderCircle
            :size="14"
            class="shell-project-menu__spinner"
            aria-hidden="true"
          />
          <span>{{
            t("shell.app.project.opening", { name: switchingTo?.name ?? "" })
          }}</span>
        </p>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
/* ── 触发器 ───────────────────────────────────────────────────────────── */

/* header 变体：左侧导航的项目上下文头（Figma Project Context Header / 40px） */
.shell-project-switcher.is-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  height: var(--size-control-lg);
  padding: 0 var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-project-switcher.is-header.is-open {
  background: var(--ws-elevated);
}

/* chip 变体：顶栏项目芯片（Figma Project switcher / 32px） */
.shell-project-switcher.is-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-compact);
  padding: 0 var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  white-space: nowrap;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-project-switcher.is-chip.is-open {
  background: var(--ws-elevated);
}

.shell-project-switcher:hover {
  background: var(--ws-elevated);
}

.shell-project-switcher:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 未选项目：标题用次文字色（Empty 态） */
.shell-project-switcher.is-empty {
  color: var(--ws-text-secondary);
}

.shell-project-switcher__icon {
  flex: none;
  color: var(--ws-text-secondary);
}

.shell-project-switcher__name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shell-project-switcher__chevron {
  flex: none;
  color: var(--ws-text-secondary);
}

/* ── 菜单 ─────────────────────────────────────────────────────────────── */

:deep(.shell-project-menu) {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  width: var(--size-project-menu-width);
  padding: var(--space-2);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

/* 搜索行 */
.shell-project-menu__search {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-row-height);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.shell-project-menu__search-input {
  flex: 1;
  min-width: 0;
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  background: transparent;
  border: none;
  outline: none;
}

.shell-project-menu__search-input::placeholder {
  color: var(--ws-text-secondary);
}

/* 项目条目（Navigation Item 36px 口径） */
.shell-project-menu__item {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-row-height);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-project-menu__item:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.shell-project-menu__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 当前项目：accent 着色面 + accent 图标 + 左侧指示条 */
.shell-project-menu__item.is-active {
  color: var(--ws-text-primary);
  font-weight: var(--weight-medium);
  background: var(--ws-color-accent-surface);
}

.shell-project-menu__item.is-active svg {
  color: var(--ws-accent);
}

.shell-project-menu__active-bar {
  position: absolute;
  top: 50%;
  left: 0;
  width: var(--size-indicator);
  height: var(--space-4);
  background: var(--ws-accent);
  border-radius: var(--radius-xs);
  transform: translateY(-50%);
  opacity: 0;
}

.shell-project-menu__item.is-active .shell-project-menu__active-bar {
  opacity: 1;
}

.shell-project-menu__item-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 切换中状态行 */
.shell-project-menu__status {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2);
  margin: 0;
  color: var(--ws-accent);
  font-size: var(--font-size-caption);
}

.shell-project-menu__spinner {
  animation: shell-project-spin var(--duration-spinner) linear infinite;
}

@keyframes shell-project-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .shell-project-menu__spinner {
    animation: none;
  }
}
</style>
