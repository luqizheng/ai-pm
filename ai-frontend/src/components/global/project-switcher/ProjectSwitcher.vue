<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/ProjectSwitcher/Trigger" node=33:87797 date=2026-11-04 -->
<script setup lang="ts">
import { computed, ref } from "vue";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger
} from "reka-ui";
import { Box, ChevronDown } from "lucide-vue-next";

import ProjectSwitcherMenu from "./ProjectSwitcherMenu.vue";
import {
  PROJECT_SWITCHER_DEFAULT_TEXT,
  type ProjectSwitcherProject,
  type ProjectSwitcherStatus,
  type ProjectSwitcherText
} from "./project-switcher";

/**
 * ProjectSwitcher —— Figma「06 — Project Context Switcher」（33:82961）：
 * Global/ProjectSwitcher/Trigger（33:87797，五态锚点）+ Menu（33:87796）。
 *
 * Trigger（36px 锚点行，口径 = TopBarItem md + 板五态覆盖）：
 *   - Default：--ws-sidebar 底（顶栏同色，视觉融入）
 *   - Hover：--surface-raised / Focus：2px accent 描边
 *   - Selected：--accent-surface（= 当前项目，菜单展开亦保持）
 *   - Disabled：整体 0.4
 * 行为约束（Figma 板注）：Loading / Error 绝不静默切换项目上下文。
 */
interface Props {
  projects: ProjectSwitcherProject[];
  /** 锚点标签（当前项目名） */
  label?: string;
  currentId?: string | null;
  status?: ProjectSwitcherStatus;
  /** 是否允许创建项目 */
  canCreate?: boolean;
  /** Trigger Disabled 态（身份不可用时整锚点禁用） */
  disabled?: boolean;
  /** Selected 态（当前项目高亮，--accent-surface 底） */
  selected?: boolean;
  texts?: Partial<ProjectSwitcherText>;
}

const props = withDefaults(defineProps<Props>(), {
  label: "",
  currentId: null,
  status: "ready",
  canCreate: true,
  disabled: false,
  selected: false,
  texts: () => ({})
});

const emit = defineEmits<{
  select: [id: string];
  retry: [];
  create: [];
}>();

const open = ref(false);
const query = ref("");

const menuTexts = computed<ProjectSwitcherText>(() => ({
  ...PROJECT_SWITCHER_DEFAULT_TEXT,
  ...props.texts
}));

function onSelect(id: string) {
  open.value = false;
  query.value = "";
  emit("select", id);
}

function onClearQuery() {
  query.value = "";
}
</script>

<template>
  <PopoverRoot v-model:open="open">
    <!-- asChild：触发器即下面的真实 button（不额外包一层交互元素） -->
    <PopoverTrigger as-child>
      <button
        type="button"
        class="ps-trigger"
        :class="{ 'is-selected': props.selected || open }"
        :disabled="props.disabled"
      >
        <Box class="ps-trigger__icon" aria-hidden="true" />
        <span class="ps-trigger__label">{{ props.label }}</span>
        <ChevronDown
          class="ps-trigger__chevron"
          :size="16"
          aria-hidden="true"
        />
      </button>
    </PopoverTrigger>

    <PopoverPortal>
      <PopoverContent
        class="ps-popover"
        side="bottom"
        align="start"
        :side-offset="4"
      >
        <ProjectSwitcherMenu
          :projects="props.projects"
          :current-id="props.currentId"
          :status="props.status"
          v-model:query="query"
          :can-create="props.canCreate"
          :texts="props.texts"
          @select="onSelect"
          @retry="emit('retry')"
          @create="emit('create')"
          @clear-query="onClearQuery"
        />
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
/* 锚点（Figma Trigger：h36 / padding 0 12 / gap 8 / radius 8） */
.ps-trigger {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  max-width: 100%;
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  border: none;
  border-radius: var(--radius-sm);
  background-color: var(--ws-sidebar);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    opacity var(--duration-fast) var(--ease-decelerate);
}

/* Hover / Focus / Selected / Disabled（Figma 板五态） */
.ps-trigger:hover:not(:disabled) {
  background-color: var(--surface-raised);
}

.ps-trigger:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 0;
}

.ps-trigger.is-selected {
  background-color: var(--accent-surface);
}

.ps-trigger:disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}

.ps-trigger__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

.ps-trigger__label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.ps-trigger__chevron {
  flex: none;
  color: var(--text-secondary);
}

/* 浮层定位壳：面板 chrome（surface 底 / 边框 / 内边距）由 Menu 根承担 */
.ps-popover {
  z-index: 50;
  outline: none;
}
</style>
