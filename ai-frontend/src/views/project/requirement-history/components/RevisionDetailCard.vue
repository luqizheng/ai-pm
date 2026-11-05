<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Link2 } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type {
  RequirementEntry,
  RevisionRecord
} from "../../requirements/requirement-entry";

/**
 * 修订详情卡（S02-UI-05 §4 + Deliverable 2）。
 *
 * 选中修订时显示：需求标题 / 该版正文（轻量 Markdown 行渲染）/
 * 类型与优先级（该版快照，缺失回退当前值） / 该版验收标准快照 /
 * 关联来源引用（引用需求的来源证据 ref，只读）。
 */
interface Props {
  revision: RevisionRecord | null;
  entry: RequirementEntry;
}

const props = defineProps<Props>();
const { t } = useI18n();

interface MarkdownLine {
  kind: "h1" | "li" | "quote" | "p";
  text: string;
}

/** 轻量 Markdown 行渲染：fixtures 正文只含标题/列表/引用/段落四种形态 */
function parseMarkdown(markdown: string): MarkdownLine[] {
  return markdown
    .split("\n")
    .filter(line => line.trim().length > 0)
    .map(line => {
      if (line.startsWith("# "))
        return { kind: "h1" as const, text: line.slice(2) };
      if (line.startsWith("- "))
        return { kind: "li" as const, text: line.slice(2) };
      if (line.startsWith("> "))
        return { kind: "quote" as const, text: line.slice(2) };
      return { kind: "p" as const, text: line };
    });
}

const lines = computed(() =>
  props.revision ? parseMarkdown(props.revision.markdown) : []
);

const typeLabel = computed(() => {
  const value = props.revision?.type ?? props.entry.type;
  return value === "functional"
    ? t("pm.requirements.typeFunctional")
    : t("pm.requirements.typeNonFunctional");
});

const priorityLabel = computed(
  () =>
    (
      ({
        high: t("pm.requirements.priorityHigh"),
        medium: t("pm.requirements.priorityMedium"),
        low: t("pm.requirements.priorityLow")
      }) as const
    )[props.revision?.priority ?? props.entry.priority]
);

const criteria = computed(() => props.revision?.criteria ?? []);

const sourceRefs = computed(() =>
  props.entry.sources.map(source => ({
    id: source.id,
    ref: source.ref,
    title: source.title
  }))
);
</script>

<template>
  <section class="rhd" :aria-label="t('pm.requirementHistory.detailTitle')">
    <h3 class="rhd__title">{{ t("pm.requirementHistory.detailTitle") }}</h3>

    <EmptyState
      v-if="!props.revision"
      size="sm"
      :title="t('pm.requirementHistory.detailEmptyTitle')"
      :description="t('pm.requirementHistory.detailEmptyDesc')"
    />

    <template v-else>
      <div class="rhd__head">
        <span class="rhd__requirement-title">{{ props.entry.title }}</span>
        <Badge variant="neutral" size="sm">{{ typeLabel }}</Badge>
        <Badge variant="neutral" size="sm">{{ priorityLabel }}</Badge>
      </div>

      <div class="rhd__markdown">
        <p
          v-for="(line, index) in lines"
          :key="index"
          class="rhd__line"
          :class="`rhd__line--${line.kind}`"
        >
          {{ line.text }}
        </p>
      </div>

      <div class="rhd__section">
        <span class="rhd__section-label">
          {{ t("pm.requirementDetail.acceptTitle") }}
        </span>
        <ol v-if="criteria.length > 0" class="rhd__criteria">
          <li v-for="criterion in criteria" :key="criterion.id">
            {{ criterion.statement }}
          </li>
        </ol>
        <p v-else class="rhd__empty">
          {{ t("pm.requirementHistory.detailSnapshotUnavailable") }}
        </p>
      </div>

      <div class="rhd__section">
        <span class="rhd__section-label">
          {{ t("pm.requirementHistory.detailSourceRefs") }}
        </span>
        <ul v-if="sourceRefs.length > 0" class="rhd__refs">
          <li v-for="source in sourceRefs" :key="source.id">
            <Link2 class="rhd__ref-icon" aria-hidden="true" />
            <span v-if="source.ref" class="rhd__ref-code">{{
              source.ref
            }}</span>
            <span class="rhd__ref-title">{{ source.title }}</span>
          </li>
        </ul>
        <p v-else class="rhd__empty">{{ t("pm.requirements.sourcesEmpty") }}</p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.rhd {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rhd__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rhd__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rhd__requirement-title {
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rhd__markdown {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
  background: var(--surface-raised);
}

.rhd__line {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rhd__line--h1 {
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rhd__line--li {
  padding-left: var(--space-4);
  position: relative;
}

.rhd__line--li::before {
  content: "·";
  position: absolute;
  left: var(--space-2);
  color: var(--text-secondary);
}

.rhd__line--quote {
  border-left: var(--border-2) solid var(--border-default);
  padding-left: var(--space-2);
  color: var(--text-secondary);
}

.rhd__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rhd__section-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.rhd__criteria {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding-left: var(--space-5);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rhd__refs {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rhd__refs li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rhd__ref-icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  color: var(--text-disabled);
}

.rhd__ref-code {
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.rhd__ref-title {
  color: var(--text-primary);
}

.rhd__empty {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}
</style>
