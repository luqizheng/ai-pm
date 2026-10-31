<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FallbackPanel from "../registry/FallbackPanel.vue";
import { getWorkspaceComponent } from "../registry/registry";
import type { WorkspacePanel, WorkspaceSpec } from "../spec/types";
import { resolveSlots } from "./templates";

const props = defineProps<{ spec: WorkspaceSpec }>();
const { t } = useI18n();
const resolved = computed(() => resolveSlots(props.spec));

function def(panel: WorkspacePanel) {
  return getWorkspaceComponent(panel.component);
}
</script>

<template>
  <div
    class="ws-canvas"
    :class="`ws-canvas--${spec.layout}`"
    data-test="workspace-canvas"
  >
    <section
      v-for="(slot, slotIndex) in resolved.slots"
      :key="slotIndex"
      class="ws-canvas__slot"
      :data-test="`canvas-slot-${slotIndex}`"
    >
      <template v-for="(panel, panelIndex) in slot" :key="`${panel.component}-${panelIndex}`">
        <component
          :is="def(panel)!.component"
          v-if="def(panel)"
          v-bind="panel.config ?? {}"
        />
        <FallbackPanel
          v-else
          :reason="`unknown-component:${panel.component}`"
          :data-ref="panel.dataRef?.id"
        />
      </template>
    </section>

    <details
      v-if="resolved.overflow.length"
      class="ws-canvas__overflow"
      data-test="canvas-overflow"
    >
      <summary class="ws-canvas__summary">
        {{ t("workspace.spec.overflow") }}（{{ resolved.overflow.length }}）
      </summary>
      <div class="ws-canvas__overflow-body">
        <template
          v-for="(panel, index) in resolved.overflow"
          :key="`overflow-${panel.component}-${index}`"
        >
          <component
            :is="def(panel)!.component"
            v-if="def(panel)"
            v-bind="panel.config ?? {}"
          />
          <FallbackPanel
            v-else
            :reason="`unknown-component:${panel.component}`"
            :data-ref="panel.dataRef?.id"
          />
        </template>
      </div>
    </details>
  </div>
</template>

<style scoped>
.ws-canvas {
  display: grid;
  gap: 12px;
}

.ws-canvas--overview,
.ws-canvas--explorer {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.ws-canvas--split {
  grid-template-columns: minmax(0, 2fr) minmax(0, 1fr);
}

.ws-canvas--timeline {
  grid-template-columns: minmax(0, 1fr);
}

.ws-canvas__slot {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
}

.ws-canvas__overflow {
  grid-column: 1 / -1;
  border: 1px dashed var(--ws-border);
  border-radius: var(--ws-radius-card);
  padding: 8px 10px;
}

.ws-canvas__summary {
  cursor: pointer;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-canvas__overflow-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 8px;
}
</style>