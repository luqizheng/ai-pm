<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Search/CommandPalette + Query + Results" node=33:87787;33:87788;33:87789 date=2026-11-04 -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Check,
  Folder,
  GitBranch,
  Inbox,
  Plus,
  SearchX,
  User
} from "lucide-vue-next";
import type { Component } from "vue";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";
import type {
  RecentSearch,
  SearchCommand,
  SearchFn,
  SearchObjectType,
  SearchResultItem,
  SearchResultState
} from "./types";
import { SEARCH_OBJECT_TYPES } from "./types";
import SearchInput from "./SearchInput.vue";
import SearchRecentPanel from "./SearchRecentPanel.vue";
import SearchResultRow from "./SearchResultRow.vue";

/**
 * 全局搜索 / 命令面板（Figma「01 — Global Search」+「02 — Search Results」，
 * Global/Search/CommandPalette · Query · Results 三族）。
 *
 * ⌘K / Ctrl+K 打开的唯一权限感知入口：六类 R&D 对象 + 显式全局命令。
 * 状态机（Query 三态 + Results 三态）：
 *   idle        —— 命令分组 + 最近搜索（无查询）
 *   searching   —— 进度条 + 骨架行（查询进行中，恒有进度反馈）
 *   results     —— 计数行 + 六类分组结果（清除查询按钮）
 *   no-results  —— 空态（清空查询动作）
 *   error       —— danger 反馈框 + 重试（查询保留）
 *
 * 键盘：↑ ↓ 在可选项间移动（键盘焦点 = 2px accent 描边，与选中态独立）；
 * Enter 打开（select 目标路由）并关闭；Esc 关闭。
 */
interface Props {
  /** 是否打开（v-model:open） */
  open?: boolean;
  /** 全局命令 */
  commands?: SearchCommand[];
  /** 最近搜索 */
  recentSearches?: RecentSearch[];
  /** 异步搜索函数（rejected → Error 态） */
  searchFn: SearchFn;
  /** 当前已选中对象 id（Selected 态；与键盘焦点互相独立） */
  selectedId?: string | null;
  /** 输入防抖毫秒数 */
  debounceMs?: number;
}

const props = withDefaults(defineProps<Props>(), {
  open: false,
  commands: () => [],
  recentSearches: () => [],
  selectedId: null,
  debounceMs: 150
});

const emit = defineEmits<{
  "update:open": [value: boolean];
  /** 打开选中的命令 / 结果（target 路由路径） */
  select: [target: string];
}>();

const { t } = useI18n();

type Status = "idle" | "searching" | "results" | "no-results" | "error";

const query = ref("");
const status = ref<Status>("idle");
const results = ref<SearchResultItem[]>([]);
const activeIndex = ref(0);

let searchSeq = 0;
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

const COMMAND_ICONS: Record<NonNullable<SearchCommand["icon"]>, Component> = {
  folder: Folder,
  gitBranch: GitBranch,
  plus: Plus,
  user: User,
  inbox: Inbox
};

/** 主面板命令（navigate + create 分组）与附加面板命令（action 分组） */
const mainCommands = computed(() =>
  props.commands.filter(c => c.group !== "action")
);
const actionCommands = computed(() =>
  props.commands.filter(c => c.group === "action")
);
const createCommands = computed(() =>
  mainCommands.value.filter(c => c.group === "create")
);
const navigateCommands = computed(() =>
  mainCommands.value.filter(c => c.group === "navigate")
);

/** 键盘导航的扁平可选项：idle = 全部命令；results = 全部结果 */
const actionableItems = computed<string[]>(() => {
  if (status.value === "results") {
    return results.value.map(item => item.id);
  }
  if (status.value === "idle") {
    return [
      ...navigateCommands.value,
      ...createCommands.value,
      ...actionCommands.value
    ].map(c => c.id);
  }
  return [];
});

/** 结果按六类分组（空类不展示，顺序固定） */
const groupedResults = computed(() => {
  const groups: { type: SearchObjectType; items: SearchResultItem[] }[] = [];
  for (const type of SEARCH_OBJECT_TYPES) {
    const items = results.value.filter(item => item.objectType === type);
    if (items.length > 0) {
      groups.push({ type, items });
    }
  }
  return groups;
});

function itemState(id: string): SearchResultState {
  // 键盘焦点视觉优先（键盘导航中）；Selected 与 Keyboard focus 互相独立（Figma 口径）
  const flat = actionableItems.value;
  const index = flat.indexOf(id);
  if (index !== -1 && index === activeIndex.value) return "keyboardFocus";
  if (props.selectedId === id) return "selected";
  return "default";
}

/** 命令行态：键盘焦点行 = accent 着色面 + ↵ 芯片（Figma Menu option Active） */
function commandState(id: string): "default" | "active" {
  return actionableItems.value[activeIndex.value] === id &&
    status.value === "idle"
    ? "active"
    : "default";
}

function runSearch(q: string) {
  const seq = ++searchSeq;
  status.value = "searching";
  props
    .searchFn(q)
    .then(items => {
      if (seq !== searchSeq) return;
      results.value = items;
      status.value = items.length > 0 ? "results" : "no-results";
    })
    .catch(() => {
      if (seq !== searchSeq) return;
      status.value = "error";
    });
}

watch(query, q => {
  if (debounceTimer) clearTimeout(debounceTimer);
  const trimmed = q.trim();
  if (!trimmed) {
    searchSeq += 1; // 使在途请求失效
    status.value = "idle";
    results.value = [];
    return;
  }
  debounceTimer = setTimeout(() => runSearch(trimmed), props.debounceMs);
});

const rootRef = ref<HTMLElement | null>(null);

watch(
  () => props.open,
  async open => {
    if (debounceTimer) clearTimeout(debounceTimer);
    query.value = "";
    results.value = [];
    status.value = "idle";
    activeIndex.value = 0;
    if (open) {
      await nextTick();
      panelInput()?.focus();
    }
  }
);

/** 面板根下的搜索输入框（autofocus 用） */
function panelInput(): HTMLInputElement | null {
  return rootRef.value?.querySelector<HTMLInputElement>("input") ?? null;
}

function close() {
  emit("update:open", false);
}

function onSelectCommand(command: SearchCommand) {
  close();
  emit("select", command.target);
}

function onSelectItem(item: SearchResultItem) {
  close();
  emit("select", item.target);
}

function onRecentSelect(item: RecentSearch) {
  query.value = item.label;
}

function clearQuery() {
  query.value = "";
}

function retry() {
  const trimmed = query.value.trim();
  if (trimmed) runSearch(trimmed);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    close();
    return;
  }
  if (event.key === "ArrowDown") {
    event.preventDefault();
    const max = actionableItems.value.length - 1;
    if (max >= 0) activeIndex.value = Math.min(activeIndex.value + 1, max);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    activeIndex.value = Math.max(activeIndex.value - 1, 0);
  } else if (event.key === "Enter") {
    event.preventDefault();
    const id = actionableItems.value[activeIndex.value];
    if (!id) return;
    const command = props.commands.find(c => c.id === id);
    if (command) {
      onSelectCommand(command);
      return;
    }
    const item = results.value.find(r => r.id === id);
    if (item) onSelectItem(item);
  }
}

onBeforeUnmount(() => {
  if (debounceTimer) clearTimeout(debounceTimer);
});
</script>

<template>
  <div
    v-if="props.open"
    ref="rootRef"
    class="global-search"
    role="dialog"
    aria-modal="true"
    :aria-label="t('global.search.label')"
    @keydown="onKeydown"
  >
    <div class="global-search__scrim" aria-hidden="true" @click="close" />

    <div class="global-search__panel">
      <!-- 核心面板：搜索输入 + 命令分组 / 搜索中反馈 -->
      <section class="global-search__core">
        <SearchInput
          v-model="query"
          :placeholder="t('global.search.placeholder')"
          :loading="status === 'searching'"
          escape-chip
        />

        <!-- idle：全局命令分组（Figma GLOBAL COMMANDS / CREATE） -->
        <template v-if="status === 'idle'">
          <template v-if="navigateCommands.length">
            <p class="global-search__group-label">
              {{ t("global.search.groupCommands") }}
            </p>
            <button
              v-for="command in navigateCommands"
              :key="command.id"
              type="button"
              class="global-search__option"
              :class="{ 'is-active': commandState(command.id) === 'active' }"
              @click="onSelectCommand(command)"
            >
              <component
                :is="COMMAND_ICONS[command.icon ?? 'folder']"
                :size="16"
                aria-hidden="true"
              />
              <span class="global-search__option-label">{{
                command.label
              }}</span>
              <kbd
                v-if="commandState(command.id) === 'active'"
                class="global-search__kbd"
                >↵</kbd
              >
              <Check
                v-if="commandState(command.id) === 'active'"
                :size="14"
                aria-hidden="true"
              />
            </button>
          </template>
          <template v-if="createCommands.length">
            <p class="global-search__group-label">
              {{ t("global.search.groupCreate") }}
            </p>
            <button
              v-for="command in createCommands"
              :key="command.id"
              type="button"
              class="global-search__option"
              :class="{ 'is-active': commandState(command.id) === 'active' }"
              @click="onSelectCommand(command)"
            >
              <component
                :is="COMMAND_ICONS[command.icon ?? 'plus']"
                :size="16"
                aria-hidden="true"
              />
              <span class="global-search__option-label">{{
                command.label
              }}</span>
              <kbd
                v-if="commandState(command.id) === 'active'"
                class="global-search__kbd"
                >↵</kbd
              >
              <Check
                v-if="commandState(command.id) === 'active'"
                :size="14"
                aria-hidden="true"
              />
            </button>
          </template>
        </template>

        <!-- searching：进度反馈 + 骨架行 -->
        <template v-else-if="status === 'searching'">
          <div class="global-search__progress" role="status">
            <p class="global-search__progress-label">
              {{ t("global.search.searching") }}
            </p>
            <div class="global-search__progress-track" aria-hidden="true">
              <div class="global-search__progress-fill" />
            </div>
          </div>
          <div
            v-for="n in 2"
            :key="n"
            class="global-search__skeleton-row"
            aria-hidden="true"
          >
            <Skeleton class="global-search__skeleton-line" tone="strong" />
            <Skeleton class="global-search__skeleton-line" tone="base" />
          </div>
        </template>
      </section>

      <!-- error：查询保留 + 重试（Figma Query/Error） -->
      <template v-if="status === 'error'">
        <Alert variant="danger" :title="t('global.search.errorTitle')">
          {{ t("global.search.errorDesc", { query: query.trim() }) }}
        </Alert>
        <div>
          <Button variant="secondary" size="sm" @click="retry">
            {{ t("global.search.retry") }}
          </Button>
        </div>
      </template>

      <!-- idle：附加命令面板 + 最近搜索 -->
      <template v-if="status === 'idle'">
        <section
          v-if="actionCommands.length"
          class="global-search__aux"
          :aria-label="t('global.search.groupCommands')"
        >
          <button
            v-for="command in actionCommands"
            :key="command.id"
            type="button"
            class="global-search__option is-aux"
            role="menuitem"
            @click="onSelectCommand(command)"
          >
            <component
              :is="COMMAND_ICONS[command.icon ?? 'inbox']"
              :size="16"
              aria-hidden="true"
            />
            <span class="global-search__option-label">{{ command.label }}</span>
          </button>
        </section>

        <SearchRecentPanel
          v-if="props.recentSearches.length"
          :items="props.recentSearches"
          @select="onRecentSelect"
        />
        <section v-else class="global-search__empty">
          <SearchX :size="24" aria-hidden="true" />
          <p class="global-search__empty-title">
            {{ t("global.search.noRecentTitle") }}
          </p>
          <p class="global-search__empty-desc">
            {{ t("global.search.noRecentDesc") }}
          </p>
        </section>
      </template>

      <!-- results：计数 + 六类分组（Figma「02 — Search Results」） -->
      <section
        v-else-if="status === 'results'"
        class="global-search__results"
        role="listbox"
        :aria-label="t('global.search.label')"
      >
        <div class="global-search__query-row">
          <SearchInput
            v-model="query"
            :placeholder="t('global.search.placeholder')"
          />
          <Button variant="secondary" size="sm" @click="clearQuery">
            {{ t("global.search.clearQuery") }}
          </Button>
        </div>
        <p class="global-search__count">
          {{ t("global.search.countResults", { count: results.length }) }}
        </p>
        <div class="global-search__categories">
          <div
            v-for="group in groupedResults"
            :key="group.type"
            class="global-search__category"
          >
            <p class="global-search__group-label">
              {{ t(`global.search.category.${group.type}`) }}
              · {{ group.items.length }}
            </p>
            <SearchResultRow
              v-for="item in group.items"
              :key="item.id"
              :item="item"
              :state="itemState(item.id)"
              @select="onSelectItem"
            />
          </div>
        </div>
      </section>

      <!-- no-results：空态 + 清空查询 -->
      <section v-else-if="status === 'no-results'" class="global-search__empty">
        <SearchX :size="24" aria-hidden="true" />
        <p class="global-search__empty-title">
          {{ t("global.search.noResultsTitle") }}
        </p>
        <p class="global-search__empty-desc">
          {{ t("global.search.noResultsDesc") }}
        </p>
        <Button variant="secondary" size="sm" @click="clearQuery">
          {{ t("global.search.clearQuery") }}
        </Button>
      </section>

      <!-- 键盘提示条（两帧共有的 Search keyboard guidance） -->
      <footer class="global-search__hints" aria-hidden="true">
        <span>{{ t("global.search.hintSelect") }}</span>
        <span>{{ t("global.search.hintOpen") }}</span>
        <span>{{ t("global.search.hintClose") }}</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
/* 遮罩 28% 透明做进背景色（写在元素 opacity 上会连面板文字一起压透明） */
.global-search {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 15vh;
  background: color-mix(in srgb, var(--ws-canvas) 28%, transparent);
}

.global-search__panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  width: calc(100% - var(--space-8));
  max-width: var(--size-search-palette-width);
  max-height: 70vh;
  overflow-y: auto;
}

/* 核心面板（Figma Core command palette：elevated 底 / radius 8 / padding 8） */
.global-search__core {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.global-search__group-label {
  margin: 0;
  padding: 0 var(--space-1);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  letter-spacing: 0.08em;
}

/* 命令行（Figma Menu option：h36 / radius 8 / padding 0 8） */
.global-search__option {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-row-height);
  padding: 0 var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.global-search__option:hover {
  background: var(--ws-hover-veil);
}

.global-search__option:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* 键盘焦点行：accent 着色面 + ↵ 芯片 + check */
.global-search__option.is-active {
  background: var(--ws-color-accent-surface);
}

.global-search__option-label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.global-search__kbd {
  padding: 1px var(--space-1);
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
}

/* 附加命令面板（Figma Additional commands：surface 底独立面板） */
.global-search__aux {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-2);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

/* 搜索中：进度反馈（label + 6px 不定进度轨道） */
.global-search__progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.global-search__progress-label {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-micro);
}

.global-search__progress-track {
  position: relative;
  height: var(--size-progress-track);
  overflow: hidden;
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

.global-search__progress-fill {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 30%;
  background: var(--ws-accent);
  border-radius: var(--radius-xs);
  animation: global-search-progress var(--duration-progress) var(--ease-standard) infinite;
}

@keyframes global-search-progress {
  from {
    left: -30%;
  }

  to {
    left: 100%;
  }
}

/* 骨架行（h36 行形容器内双色占位线） */
.global-search__skeleton-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-row-height);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.global-search__skeleton-line {
  height: var(--space-2);
}

.global-search__skeleton-line:first-child {
  width: 40%;
}

.global-search__skeleton-line:last-child {
  width: 28%;
}

/* 结果面板（Figma Search results：surface 底 / padding 16 / gap 12） */
.global-search__results {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.global-search__query-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.global-search__query-row > :first-child {
  flex: 1;
}

.global-search__count {
  margin: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.global-search__categories {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.global-search__category {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

/* 空态块（Figma EmptyState：居中 图标24 / 标题13 SB / 描述11） */
.global-search__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-6);
  color: var(--ws-text-secondary);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.global-search__empty-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
}

.global-search__empty-desc {
  margin: 0;
  font-size: var(--font-size-micro);
  text-align: center;
}

/* 键盘提示条（Figma Search keyboard guidance：sidebar 底 / mono 11） */
.global-search__hints {
  display: flex;
  gap: var(--space-5);
  padding: var(--space-3);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  background: var(--ws-sidebar);
}

@media (prefers-reduced-motion: reduce) {
  .global-search__progress-fill {
    animation: none;
  }
}
</style>
