<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  BookOpen,
  Flag,
  GitBranch,
  ListChecks,
  Network,
  ShieldCheck,
  Waypoints
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import type {
  ChangeImpactAnalysis,
  ImpactLevel,
  ImpactTarget,
  ImpactTargetKind
} from "../impact-contract";

/**
 * 依赖可视化（S02-UI-07 §4 + Deliverable 2）。
 *
 * 中心节点 = 拟议修订；右侧 = 受影响对象（按类型节点 + 影响级别徽标 +
 * 影响原因）。四级可辨：direct（accent）/ indirect（info）/ potential
 * （warning）/ uncertain（neutral 弱化）。**不虚构依赖**：全部节点来自
 * 需求域真实关系（fixtures 逐条可溯），页面明示该口径。
 */
interface Props {
  analysis: ChangeImpactAnalysis;
}

const props = defineProps<Props>();
const { t } = useI18n();

const kindIcons: Record<ImpactTargetKind, Component> = {
  acceptance_criteria: ListChecks,
  task: GitBranch,
  knowledge_doc: BookOpen,
  baseline: ShieldCheck,
  milestone: Flag
};

const levelMeta: Record<
  ImpactLevel,
  {
    key: string;
    variant: "accent" | "info" | "warning" | "neutral";
    outline: boolean;
  }
> = {
  direct: { key: "levelDirect", variant: "accent", outline: false },
  indirect: { key: "levelIndirect", variant: "info", outline: false },
  potential: { key: "levelPotential", variant: "warning", outline: false },
  uncertain: { key: "levelUncertain", variant: "neutral", outline: true }
};

const IMPACT_LEVELS: ImpactLevel[] = [
  "direct",
  "indirect",
  "potential",
  "uncertain"
];

const kindLabel = (kind: ImpactTargetKind) =>
  ({
    acceptance_criteria: t("pm.requirementImpact.kindCriteria"),
    task: t("pm.requirementImpact.kindTask"),
    knowledge_doc: t("pm.requirementImpact.kindKnowledge"),
    baseline: t("pm.requirementImpact.kindBaseline"),
    milestone: t("pm.requirementImpact.kindMilestone")
  })[kind];

const targets = computed<ImpactTarget[]>(() => props.analysis.targets);
</script>

<template>
  <section class="rdg" :aria-label="t('pm.requirementImpact.graphTitle')">
    <div class="rdg__head">
      <h3 class="rdg__title">
        <Network class="rdg__title-icon" aria-hidden="true" />
        {{ t("pm.requirementImpact.graphTitle") }}
      </h3>
      <Waypoints class="rdg__head-icon" aria-hidden="true" />
    </div>

    <div class="rdg__graph">
      <div class="rdg__center">
        <div class="rdg__center-node">
          <span class="rdg__center-code">{{
            props.analysis.requirement_code
          }}</span>
          <span class="rdg__center-title">{{ props.analysis.title }}</span>
          <Badge variant="accent" size="sm">
            {{
              t("pm.requirements.revisionVersion", {
                version: props.analysis.proposed_revision
              })
            }}
          </Badge>
          <span class="rdg__center-role">{{
            t("pm.requirementImpact.graphSourceNode")
          }}</span>
        </div>
      </div>

      <div class="rdg__link-col" aria-hidden="true">
        <span
          v-for="target in targets"
          :key="`link-${target.id}`"
          class="rdg__link"
          :class="`rdg__link--${target.level}`"
        />
      </div>

      <ul class="rdg__targets">
        <li
          v-for="target in targets"
          :key="target.id"
          class="rdg__target"
          :class="`rdg__target--${target.level}`"
        >
          <div class="rdg__target-head">
            <component
              :is="kindIcons[target.kind]"
              class="rdg__target-icon"
              aria-hidden="true"
            />
            <span class="rdg__target-kind">{{ kindLabel(target.kind) }}</span>
            <span class="rdg__target-code">{{ target.code }}</span>
            <Badge
              :variant="levelMeta[target.level].variant"
              size="sm"
              :outline="levelMeta[target.level].outline"
            >
              {{ t(`pm.requirementImpact.${levelMeta[target.level].key}`) }}
            </Badge>
          </div>
          <span class="rdg__target-title">{{ target.title }}</span>
          <p class="rdg__target-reason">{{ target.reason }}</p>
        </li>
      </ul>
    </div>

    <div class="rdg__legend" role="list">
      <span
        v-for="level in IMPACT_LEVELS"
        :key="level"
        class="rdg__legend-item"
        role="listitem"
      >
        <Badge
          :variant="levelMeta[level].variant"
          size="sm"
          :outline="levelMeta[level].outline"
        >
          {{ t(`pm.requirementImpact.${levelMeta[level].key}`) }}
        </Badge>
      </span>
    </div>
    <p class="rdg__note">{{ t("pm.requirementImpact.graphNoInventNote") }}</p>
  </section>
</template>

<style scoped>
.rdg {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rdg__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.rdg__title {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rdg__title-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}

.rdg__head-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-disabled);
}

.rdg__graph {
  display: grid;
  grid-template-columns: minmax(0, 30%) auto minmax(0, 1fr);
  gap: 0;
  align-items: stretch;
  min-width: 0;
}

.rdg__center {
  display: flex;
  align-items: center;
  min-width: 0;
  padding-right: var(--space-2);
}

.rdg__center-node {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-1);
  width: 100%;
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-sm);
  background: var(--ws-color-accent-surface);
  padding: var(--space-3);
}

.rdg__center-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rdg__center-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.rdg__center-role {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

/* 连线列：与目标节点逐行对齐的影响线 */
.rdg__link-col {
  display: flex;
  flex-direction: column;
  justify-content: space-around;
  padding: 0 var(--space-1);
}

.rdg__link {
  flex: 1 1 0;
  min-height: var(--space-6);
  width: 0;
  border-left: var(--border-1) solid var(--border-default);
}

.rdg__link--direct {
  border-left: var(--border-2) solid var(--accent);
}

.rdg__link--indirect {
  border-left: var(--border-1) solid var(--ws-color-info);
}

.rdg__link--potential {
  border-left: var(--border-1) dashed var(--ws-color-warning);
}

.rdg__link--uncertain {
  border-left: var(--border-1) dotted var(--border-default);
}

.rdg__targets {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
  min-width: 0;
}

.rdg__target {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  flex: 1 1 0;
  min-width: 0;
}

.rdg__target--uncertain {
  border-style: dashed;
}

.rdg__target-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rdg__target-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-secondary);
}

.rdg__target-kind {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rdg__target-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rdg__target-head > :last-child {
  margin-left: auto;
}

.rdg__target-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.rdg__target-reason {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rdg__legend {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rdg__note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

@media (max-width: 1100px) {
  .rdg__graph {
    grid-template-columns: minmax(0, 1fr);
  }

  .rdg__link-col {
    display: none;
  }
}
</style>
