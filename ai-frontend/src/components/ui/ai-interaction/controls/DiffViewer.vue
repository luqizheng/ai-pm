<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, ChevronRight } from "lucide-vue-next";

import type { DiffLine, DiffPayload } from "../types";

/**
 * Unified diff rendering (context / add / remove) with old & new line numbers.
 * The hunk header `@@ -12,7 +12,9 @@` is the only source of the line starts.
 */
const props = withDefaults(
  defineProps<{
    diff: DiffPayload;
    collapsed?: boolean;
  }>(),
  {
    collapsed: undefined
  }
);

const { t } = useI18n();

const isCollapsed = ref(props.collapsed ?? false);

watch(
  () => props.collapsed,
  value => {
    if (value !== undefined) isCollapsed.value = value;
  }
);

interface RenderedLine extends DiffLine {
  oldNumber: string;
  newNumber: string;
  marker: string;
}

interface RenderedHunk {
  header: string;
  lines: RenderedLine[];
}

const MARKERS: Record<DiffLine["kind"], string> = {
  context: " ",
  add: "+",
  remove: "-"
};

const LINE_START = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

const hunks = computed<RenderedHunk[]>(() =>
  props.diff.hunks.map(hunk => {
    const match = LINE_START.exec(hunk.header);
    let oldCursor = match ? Number(match[1]) : 0;
    let newCursor = match ? Number(match[2]) : 0;

    const lines = hunk.lines.map(line => {
      let oldNumber = "";
      let newNumber = "";

      if (line.kind === "remove") {
        oldNumber = String(oldCursor);
        oldCursor += 1;
      } else if (line.kind === "add") {
        newNumber = String(newCursor);
        newCursor += 1;
      } else {
        oldNumber = String(oldCursor);
        newNumber = String(newCursor);
        oldCursor += 1;
        newCursor += 1;
      }

      return {
        ...line,
        oldNumber,
        newNumber,
        marker: MARKERS[line.kind]
      };
    });

    return { header: hunk.header, lines };
  })
);

const addedCount = computed(() =>
  props.diff.hunks.reduce(
    (total, hunk) =>
      total + hunk.lines.filter(line => line.kind === "add").length,
    0
  )
);

const removedCount = computed(() =>
  props.diff.hunks.reduce(
    (total, hunk) =>
      total + hunk.lines.filter(line => line.kind === "remove").length,
    0
  )
);

function toggle() {
  isCollapsed.value = !isCollapsed.value;
}
</script>

<template>
  <section class="ai-diff" :class="{ 'is-collapsed': isCollapsed }">
    <header class="ai-diff__header">
      <span class="ai-diff__path" :title="diff.path">{{ diff.path }}</span>
      <span class="ai-diff__stat ai-diff__stat--add">
        {{ t("ai.controls.diff.added", { count: addedCount }) }}
      </span>
      <span class="ai-diff__stat ai-diff__stat--remove">
        {{ t("ai.controls.diff.removed", { count: removedCount }) }}
      </span>
      <button
        type="button"
        class="ai-diff__toggle"
        :aria-expanded="!isCollapsed"
        @click="toggle"
      >
        <component
          :is="isCollapsed ? ChevronRight : ChevronDown"
          :size="14"
          aria-hidden="true"
        />
        <span>{{
          isCollapsed
            ? t("ai.controls.diff.expand")
            : t("ai.controls.diff.collapse")
        }}</span>
      </button>
    </header>

    <div v-if="!isCollapsed" class="ai-diff__body">
      <div
        v-for="(hunk, hunkIndex) in hunks"
        :key="hunkIndex"
        class="ai-diff__hunk"
      >
        <div class="ai-diff__hunk-header">{{ hunk.header }}</div>
        <div
          v-for="(line, lineIndex) in hunk.lines"
          :key="lineIndex"
          class="ai-diff__line"
          :class="`is-${line.kind}`"
        >
          <span class="ai-diff__gutter">{{ line.oldNumber }}</span>
          <span class="ai-diff__gutter">{{ line.newNumber }}</span>
          <span class="ai-diff__marker" aria-hidden="true">{{
            line.marker
          }}</span>
          <span class="ai-diff__text">{{ line.text }}</span>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ai-diff {
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
  font-family: var(--font-ui);
  background: var(--surface-base);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}

.ai-diff__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-2) var(--space-1) var(--space-3);
  border-bottom: var(--border-1) solid var(--border-default);
}

.ai-diff.is-collapsed .ai-diff__header {
  border-bottom: none;
}

.ai-diff__path {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-diff__stat {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  white-space: nowrap;
}

.ai-diff__stat--add {
  color: var(--success-strong);
}

.ai-diff__stat--remove {
  color: var(--danger-strong);
}

.ai-diff__toggle {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  background: transparent;
  border: none;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.ai-diff__toggle:hover {
  color: var(--text-primary);
  background: var(--surface-raised);
}

.ai-diff__toggle:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.ai-diff__body {
  max-height: var(--size-ai-evidence-max-height);
  overflow: auto;
}

.ai-diff__hunk-header {
  padding: 0 var(--space-3);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--size-ai-diff-line);
  white-space: pre;
}

.ai-diff__line {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  min-height: var(--size-ai-diff-line);
  padding: 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--size-ai-diff-line);
  white-space: pre;
}

.ai-diff__line.is-add {
  background: var(--success-subtle);
}

.ai-diff__line.is-remove {
  background: var(--danger-surface);
}

.ai-diff__gutter {
  flex: none;
  width: var(--space-8);
  color: var(--text-secondary);
  text-align: end;
  user-select: none;
}

.ai-diff__marker {
  flex: none;
  width: var(--space-2);
  color: var(--text-secondary);
  text-align: center;
  user-select: none;
}

.ai-diff__line.is-add .ai-diff__marker {
  color: var(--success-strong);
}

.ai-diff__line.is-remove .ai-diff__marker {
  color: var(--danger-strong);
}

.ai-diff__text {
  flex: 1 1 auto;
  min-width: 0;
  color: var(--text-primary);
  overflow-wrap: anywhere;
}

@media (prefers-reduced-motion: reduce) {
  .ai-diff__toggle {
    transition: none;
  }
}
</style>
