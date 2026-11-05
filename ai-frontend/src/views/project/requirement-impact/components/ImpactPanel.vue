<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type {
  ImpactFinding,
  ImpactFindingCategory,
  ImpactLevel
} from "../impact-contract";

/**
 * 影响分析面板（S02-UI-07 §5 + Deliverable 3）。
 *
 * 发现按 A-D 四类分组：需求影响 / 实现影响 / 文档影响 / 基线影响；
 * 每条发现带影响级别徽标与关联对象编码。
 */
interface Props {
  findings: ImpactFinding[];
}

const props = defineProps<Props>();
const { t } = useI18n();

const categoryMeta: Record<
  ImpactFindingCategory,
  { badge: string; title: string }
> = {
  requirement: {
    badge: "A",
    title: "panelRequirement"
  },
  implementation: {
    badge: "B",
    title: "panelImplementation"
  },
  documentation: {
    badge: "C",
    title: "panelDocumentation"
  },
  baseline: { badge: "D", title: "panelBaseline" }
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

const groups = computed(() =>
  (Object.keys(categoryMeta) as ImpactFindingCategory[]).map(category => ({
    category,
    badge: categoryMeta[category].badge,
    title: t(`pm.requirementImpact.${categoryMeta[category].title}`),
    items: props.findings.filter(finding => finding.category === category)
  }))
);
</script>

<template>
  <section class="rip" :aria-label="t('pm.requirementImpact.panelTitle')">
    <h3 class="rip__title">{{ t("pm.requirementImpact.panelTitle") }}</h3>

    <EmptyState
      v-if="props.findings.length === 0"
      size="sm"
      :title="t('pm.requirementImpact.panelEmpty')"
    />

    <div v-else class="rip__groups">
      <article
        v-for="group in groups"
        :key="group.category"
        class="rip__group"
        :aria-label="group.title"
      >
        <div class="rip__group-head">
          <span class="rip__group-badge">{{ group.badge }}</span>
          <h4 class="rip__group-title">{{ group.title }}</h4>
          <Badge variant="neutral" size="sm">
            {{
              t("pm.requirementImpact.panelGroupCount", {
                count: group.items.length
              })
            }}
          </Badge>
        </div>
        <ul v-if="group.items.length > 0" class="rip__findings">
          <li
            v-for="finding in group.items"
            :key="finding.id"
            class="rip__finding"
          >
            <div class="rip__finding-head">
              <Badge
                :variant="levelMeta[finding.level].variant"
                size="sm"
                :outline="levelMeta[finding.level].outline"
              >
                {{ t(`pm.requirementImpact.${levelMeta[finding.level].key}`) }}
              </Badge>
              <span v-if="finding.related" class="rip__finding-related">
                {{ finding.related }}
              </span>
            </div>
            <p class="rip__finding-body">{{ finding.body }}</p>
          </li>
        </ul>
        <p v-else class="rip__group-none">
          {{ t("pm.requirementImpact.panelGroupNone") }}
        </p>
      </article>
    </div>
  </section>
</template>

<style scoped>
.rip {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rip__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rip__groups {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rip__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
}

.rip__group-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.rip__group-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-badge-sm);
  height: var(--size-badge-sm);
  border-radius: var(--radius-full);
  background: var(--surface-raised);
  font-size: var(--font-size-nano);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.rip__group-title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rip__group-head > :last-child {
  margin-left: auto;
}

.rip__findings {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rip__finding {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.rip__finding-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rip__finding-related {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rip__finding-body {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rip__group-none {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}
</style>
