<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import { useShellStore } from "@/stores/shell";
import CommandBar from "@/components/shell/CommandBar.vue";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/**
 * /design/shell/states —— Figma 板「03 — Application Shell / Shell states」。
 * 五区域尺寸契约（52/232|60/320/120|72）+ 控制状态条 + 指令栏两态实测。
 */
const { t } = useI18n();
const shell = useShellStore();

const metrics = [
  { region: "Global top bar", value: "52px" },
  { region: "Left navigation / collapsed", value: "232px / 60px" },
  { region: "Right context panel (closable)", value: "320px" },
  { region: "AI command bar expanded / collapsed", value: "120px / 72px" }
];

const controls = [
  {
    name: "Sidebar collapse",
    state: shell.sidebarCollapsed ? "collapsed" : "expanded"
  },
  {
    name: "AI panel (chat + context)",
    state: shell.aiPanelOpen ? "open" : "closed"
  },
  {
    name: "Command bar",
    state: shell.commandbarExpanded ? "expanded" : "collapsed"
  }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">{{ t("design.shell.states.title") }}</h1>
      <p class="design-page__subtitle">
        {{ t("design.shell.states.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>4px BASE / 13px BODY (board 08 footer)</h2>
      <div class="design-page__row">
        <Specimen
          v-for="m in metrics"
          :key="m.region"
          :label="m.region"
          width="300px"
        >
          <span class="shellstates__metric">{{ m.value }}</span>
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>Control state strip (board 03)</h2>
      <div class="shellstates__strip">
        <table class="shellstates__table">
          <tbody>
            <tr v-for="c in controls" :key="c.name">
              <th>{{ c.name }}</th>
              <td>{{ c.state }}</td>
              <td>
                <button
                  type="button"
                  class="shellstates__toggle"
                  @click="
                    c.name === 'Sidebar collapse'
                      ? shell.toggleSidebar()
                      : c.name === 'AI panel (chat + context)'
                        ? shell.toggleAiPanel()
                        : shell.toggleCommandbar()
                  "
                >
                  toggle
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <section class="design-page__group">
      <h2>AI command bar (shell instance, 120 / 72px)</h2>
      <Specimen label="CommandBar / live" width="720px">
        <CommandBar />
      </Specimen>
    </section>
  </DesignLayout>
</template>

<style scoped>
.design-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-page__group {
  margin: 0 0 var(--space-8);
}

.design-page__group h2 {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--space-4);
}

.shellstates__metric {
  font-family: var(--font-mono);
  font-size: var(--font-size-mono-num);
  color: var(--ws-text-primary);
}

.shellstates__strip {
  padding: var(--space-4);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  max-width: 560px;
}

.shellstates__table {
  border-collapse: collapse;
  width: 100%;
}

.shellstates__table th,
.shellstates__table td {
  padding: var(--space-2) var(--space-3) var(--space-2) 0;
  text-align: left;
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-secondary);
  border-bottom: var(--hairline) solid var(--ws-border);
}

.shellstates__table th {
  color: var(--ws-text-primary);
}

.shellstates__toggle {
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  background-color: var(--surface-raised);
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  cursor: pointer;
}

.shellstates__toggle:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}
</style>
