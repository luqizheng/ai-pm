<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch
} from "vue";
import { ChevronLeft, ChevronRight, RotateCw, X } from "lucide-vue-next";
import {
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuPortal,
  ContextMenuRoot,
  ContextMenuSeparator,
  ContextMenuTrigger
} from "reka-ui";
import { useI18n } from "vue-i18n";

export interface TabItem {
  path: string;
  title: string;
  closable?: boolean;
}

const { t } = useI18n();

interface Props {
  tabs: TabItem[];
  activePath?: string;
  closeLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  activePath: "",
  closeLabel: undefined
});

const emit = defineEmits<{
  select: [path: string];
  close: [path: string];
  reload: [path: string];
  closeOthers: [path: string];
  closeLeft: [path: string];
  closeRight: [path: string];
  closeAll: [];
}>();

const viewportRef = ref<HTMLElement | null>(null);
const overflowing = ref(false);
const canScrollPrev = ref(false);
const canScrollNext = ref(false);
const menuOpen = ref(false);
const contextPath = ref("");

const itemEls = new Map<string, HTMLElement>();

function setItemRef(path: string, el: unknown) {
  if (el) itemEls.set(path, el as HTMLElement);
  else itemEls.delete(path);
}

const contextIndex = computed(() => {
  if (!contextPath.value) return -1;
  return props.tabs.findIndex(tab => tab.path === contextPath.value);
});

const contextTab = computed(() =>
  contextIndex.value >= 0 ? props.tabs[contextIndex.value] : undefined
);

const isClosable = (tab: TabItem) => tab.closable !== false;

const canCloseCurrent = computed(
  () => !!contextTab.value && isClosable(contextTab.value)
);
const canCloseOthers = computed(
  () =>
    contextIndex.value >= 0 &&
    props.tabs.some(
      (tab, index) => index !== contextIndex.value && isClosable(tab)
    )
);
const canCloseLeft = computed(
  () =>
    contextIndex.value > 0 &&
    props.tabs.slice(0, contextIndex.value).some(isClosable)
);
const canCloseRight = computed(
  () =>
    contextIndex.value >= 0 &&
    props.tabs.slice(contextIndex.value + 1).some(isClosable)
);
const canCloseAll = computed(() => props.tabs.some(isClosable));

function updateScrollState() {
  const el = viewportRef.value;
  if (!el) return;
  const maxScroll = el.scrollWidth - el.clientWidth;
  overflowing.value = maxScroll > 1;
  canScrollPrev.value = el.scrollLeft > 1;
  canScrollNext.value = el.scrollLeft < maxScroll - 1;
}

function prefersReducedMotion() {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function scrollByPage(direction: -1 | 1) {
  const el = viewportRef.value;
  if (!el) return;
  const distance = Math.max(el.clientWidth * 0.6, 160);
  const left = direction * distance;
  if (typeof el.scrollBy === "function") {
    el.scrollBy({
      left,
      behavior: prefersReducedMotion() ? "auto" : "smooth"
    });
  } else {
    el.scrollLeft += left;
  }
  updateScrollState();
}

function ensureActiveVisible() {
  const el = itemEls.get(props.activePath);
  const viewport = viewportRef.value;
  if (!el || !viewport) return;
  const tabRect = el.getBoundingClientRect();
  const viewportRect = viewport.getBoundingClientRect();
  if (tabRect.left < viewportRect.left) {
    viewport.scrollLeft -= viewportRect.left - tabRect.left;
  } else if (tabRect.right > viewportRect.right) {
    viewport.scrollLeft += tabRect.right - viewportRect.right;
  }
  updateScrollState();
}

function onContextMenu(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const tabEl = target?.closest<HTMLElement>("[data-tab-path]");
  if (!tabEl?.dataset.tabPath) {
    // 空白处右键不弹菜单；reka 以 defaultPrevented 判定是否打开
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  contextPath.value = tabEl.dataset.tabPath;
}

let resizeObserver: ResizeObserver | undefined;

onMounted(() => {
  updateScrollState();
  ensureActiveVisible();
  window.addEventListener("resize", updateScrollState);
  document.fonts?.ready.then(updateScrollState).catch(() => {});
  if (typeof ResizeObserver !== "undefined" && viewportRef.value) {
    resizeObserver = new ResizeObserver(updateScrollState);
    resizeObserver.observe(viewportRef.value);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", updateScrollState);
  resizeObserver?.disconnect();
});

watch(
  () => props.tabs.length,
  () => nextTick(updateScrollState)
);

watch(
  () => props.activePath,
  () => nextTick(ensureActiveVisible)
);
</script>

<template>
  <div class="ui-tabbar">
    <button
      v-show="overflowing"
      type="button"
      class="ui-tabbar__nav"
      :aria-label="t('shell.tabs.scrollPrev')"
      :disabled="!canScrollPrev"
      @click="scrollByPage(-1)"
    >
      <ChevronLeft :size="16" aria-hidden="true" />
    </button>

    <ContextMenuRoot v-model:open="menuOpen">
      <ContextMenuTrigger as-child>
        <div
          ref="viewportRef"
          class="ui-tabbar__viewport"
          role="tablist"
          :aria-label="t('shell.tabs.label')"
          @scroll="updateScrollState"
          @contextmenu="onContextMenu"
        >
          <div
            v-for="tab in props.tabs"
            :key="tab.path"
            :ref="el => setItemRef(tab.path, el)"
            class="ui-tabbar__tab"
            :class="{ 'is-current': tab.path === props.activePath }"
            role="tab"
            :data-tab-path="tab.path"
            :aria-selected="tab.path === props.activePath ? 'true' : 'false'"
            :tabindex="tab.path === props.activePath ? 0 : -1"
            @click="emit('select', tab.path)"
            @keydown.enter.prevent="emit('select', tab.path)"
          >
            <span class="ui-tabbar__title">{{ tab.title }}</span>
            <button
              v-if="tab.closable !== false"
              type="button"
              class="ui-tabbar__close"
              :aria-label="props.closeLabel ?? t('shell.actions.closeTab')"
              @click.stop="emit('close', tab.path)"
            >
              <X :size="14" aria-hidden="true" />
            </button>
          </div>
        </div>
      </ContextMenuTrigger>

      <ContextMenuPortal>
        <ContextMenuContent class="ui-tabbar__menu">
          <ContextMenuItem
            class="ui-tabbar__menu-item"
            :disabled="!contextTab"
            @select="emit('reload', contextPath)"
          >
            <RotateCw :size="14" aria-hidden="true" />
            {{ t("shell.tabs.reload") }}
          </ContextMenuItem>
          <ContextMenuItem
            class="ui-tabbar__menu-item"
            :disabled="!canCloseCurrent"
            @select="emit('close', contextPath)"
          >
            <X :size="14" aria-hidden="true" />
            {{ t("shell.tabs.close") }}
          </ContextMenuItem>
          <ContextMenuSeparator class="ui-tabbar__menu-separator" />
          <ContextMenuItem
            class="ui-tabbar__menu-item"
            :disabled="!canCloseOthers"
            @select="emit('closeOthers', contextPath)"
          >
            {{ t("shell.tabs.closeOthers") }}
          </ContextMenuItem>
          <ContextMenuItem
            class="ui-tabbar__menu-item"
            :disabled="!canCloseLeft"
            @select="emit('closeLeft', contextPath)"
          >
            {{ t("shell.tabs.closeLeft") }}
          </ContextMenuItem>
          <ContextMenuItem
            class="ui-tabbar__menu-item"
            :disabled="!canCloseRight"
            @select="emit('closeRight', contextPath)"
          >
            {{ t("shell.tabs.closeRight") }}
          </ContextMenuItem>
          <ContextMenuSeparator class="ui-tabbar__menu-separator" />
          <ContextMenuItem
            class="ui-tabbar__menu-item"
            :disabled="!canCloseAll"
            @select="emit('closeAll')"
          >
            {{ t("shell.tabs.closeAll") }}
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenuPortal>
    </ContextMenuRoot>

    <button
      v-show="overflowing"
      type="button"
      class="ui-tabbar__nav"
      :aria-label="t('shell.tabs.scrollNext')"
      :disabled="!canScrollNext"
      @click="scrollByPage(1)"
    >
      <ChevronRight :size="16" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
/* 多标签页栏 — 设计文档 §5：当前项 accent-subtle 底 + accent-deep 字 + 底缘 2px accent 指示条 */
.ui-tabbar {
  display: flex;
  align-items: stretch;
  gap: var(--space-1);
  height: 36px;
  padding: 0 var(--space-3);
  background: var(--surface-raised);
  border-bottom: 1px solid var(--border-subtle);
}

/* 滚动区域：隐藏原生滚动条，改用两侧箭头翻页 */
.ui-tabbar__viewport {
  display: flex;
  flex: 1;
  align-items: stretch;
  gap: var(--space-1);
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.ui-tabbar__viewport::-webkit-scrollbar {
  display: none;
}

.ui-tabbar__nav {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  align-self: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-tabbar__nav:hover:not(:disabled) {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-tabbar__nav:disabled {
  color: var(--text-disabled);
  cursor: default;
}

.ui-tabbar__nav:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(var(--size-focus-ring) * -1);
}

.ui-tabbar__tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: 36px;
  padding: 0 var(--space-3);
  border: 0;
  border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
  white-space: nowrap;
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-tabbar__tab:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-tabbar__tab:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(var(--size-focus-ring) * -1);
}

.ui-tabbar__tab.is-current {
  background: var(--accent-subtle);
  color: var(--accent-deep);
}

.ui-tabbar__tab.is-current::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--size-indicator);
  background: var(--accent);
  content: "";
}

.ui-tabbar__title {
  overflow: hidden;
  text-overflow: ellipsis;
}

.ui-tabbar__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
}

.ui-tabbar__close:hover {
  background: var(--surface-active-veil);
  color: var(--text-primary);
}

.ui-tabbar__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

/* 右键菜单：与 DropdownMenu 浮层同一视觉口径 */
:deep(.ui-tabbar__menu) {
  min-width: calc(var(--space-12) * 4 + var(--space-2));
  padding: calc((var(--space-1) + var(--space-2)) / 2);
  background: var(--surface-overlay);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  box-shadow:
    inset 0 1px 0 0 var(--surface-highlight),
    var(--shadow-md);
  animation: ui-tabbar-menu-enter var(--duration-fast) var(--ease-decelerate);
}

:deep(.ui-tabbar__menu-item) {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  border-radius: var(--radius-xs);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
  outline: none;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

:deep(.ui-tabbar__menu-item[data-highlighted]) {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

:deep(.ui-tabbar__menu-item[data-disabled]) {
  color: var(--text-disabled);
  cursor: not-allowed;
}

:deep(.ui-tabbar__menu-separator) {
  height: 1px;
  margin: var(--space-1) 0;
  background: var(--border-subtle);
}

@keyframes ui-tabbar-menu-enter {
  from {
    opacity: 0;
    transform: scale(0.96);
  }

  to {
    opacity: 1;
    transform: scale(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  :deep(.ui-tabbar__menu) {
    animation: ui-tabbar-menu-enter var(--duration-instant) linear;
  }
}
</style>
