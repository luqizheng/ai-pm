<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  BookOpen,
  MessageSquareText,
  MessagesSquare,
  ScanSearch,
  Sparkles,
  Target
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Tabs from "@/components/ui/tabs/Tabs.vue";
import type {
  AiSuggestion,
  AnalysisFinding,
  DiscussionEntry,
  RequirementEntry,
  SourceItem
} from "../requirement-entry";
import { sortAnalysis, sortDiscussions } from "../fixtures";

/**
 * 右侧上下文面板（S02-UI-01 §7）。
 *
 * 四 Tab：Sources / Discussions / Analysis / AI Assistance。
 * AI 建议一律呈现为提案（outline 徽标），绝不自动呈现为已确认决策。
 * activeTab 受控（AI 协作栏动作可切换到 AI Assistance 页签）。
 */
interface Props {
  entry: RequirementEntry | null;
  activeTab: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{ (e: "update:activeTab", value: string): void }>();

const { t } = useI18n();

const tabs = computed(() => [
  { value: "sources", label: t("pm.requirements.tabSources") },
  { value: "discussions", label: t("pm.requirements.tabDiscussions") },
  { value: "analysis", label: t("pm.requirements.tabAnalysis") },
  { value: "ai", label: t("pm.requirements.tabAi") }
]);

const sourceIcons: Record<string, Component> = {
  user_request: MessageSquareText,
  discovery: ScanSearch,
  knowledge: BookOpen,
  discussion: MessagesSquare,
  objective: Target
};

const sortedSources = computed<SourceItem[]>(() =>
  props.entry ? props.entry.sources : []
);

const sortedDiscussions = computed<DiscussionEntry[]>(() =>
  props.entry ? sortDiscussions(props.entry.discussions) : []
);

const sortedAnalysis = computed<AnalysisFinding[]>(() =>
  props.entry ? sortAnalysis(props.entry.analysis) : []
);

const aiSuggestions = computed<AiSuggestion[]>(() =>
  props.entry ? props.entry.ai_suggestions : []
);

const analysisKindLabel = (kind: AnalysisFinding["kind"]) =>
  ({
    completeness: t("pm.requirements.analysisCompleteness"),
    ambiguity: t("pm.requirements.analysisAmbiguity"),
    duplication: t("pm.requirements.analysisDuplication"),
    conflict: t("pm.requirements.analysisConflict")
  })[kind];

const analysisKindVariant = (kind: AnalysisFinding["kind"]) =>
  kind === "conflict" ? "danger" : kind === "duplication" ? "warning" : "info";

const sourceKindLabel = (kind: string) =>
  ({
    user_request: t("pm.requirements.sourceUserRequest"),
    discovery: t("pm.requirements.sourceDiscovery"),
    knowledge: t("pm.requirements.sourceKnowledge"),
    discussion: t("pm.requirements.sourceDiscussion"),
    objective: t("pm.requirements.sourceObjective")
  })[kind] ?? kind;
</script>

<template>
  <aside class="req-context" :aria-label="t('pm.requirements.contextAria')">
    <Tabs
      :model-value="props.activeTab"
      :tabs="tabs"
      :aria-label="t('pm.requirements.contextAria')"
      @update:model-value="emit('update:activeTab', $event)"
    />

    <div v-if="!props.entry" class="req-context__empty">
      <EmptyState
        :title="t('pm.requirements.contextEmptyTitle')"
        :description="t('pm.requirements.contextEmptyDesc')"
      />
    </div>

    <template v-else>
      <div v-if="props.activeTab === 'sources'" class="req-context__panel">
        <ul v-if="sortedSources.length > 0" class="req-context__sources">
          <li
            v-for="source in sortedSources"
            :key="source.id"
            class="req-context__source"
          >
            <component
              :is="sourceIcons[source.kind]"
              class="req-context__source-icon"
              aria-hidden="true"
            />
            <div class="req-context__source-body">
              <span class="req-context__source-kind">{{
                sourceKindLabel(source.kind)
              }}</span>
              <span class="req-context__source-title">{{ source.title }}</span>
              <blockquote class="req-context__source-quote">
                {{ source.detail }}
              </blockquote>
              <span v-if="source.ref" class="req-context__source-ref">{{
                source.ref
              }}</span>
            </div>
          </li>
        </ul>
        <p v-else class="req-context__none">
          {{ t("pm.requirements.sourcesEmpty") }}
        </p>
      </div>

      <div
        v-else-if="props.activeTab === 'discussions'"
        class="req-context__panel"
      >
        <ul
          v-if="sortedDiscussions.length > 0"
          class="req-context__discussions"
        >
          <li
            v-for="discussion in sortedDiscussions"
            :key="discussion.id"
            class="req-context__discussion"
          >
            <div class="req-context__discussion-head">
              <span class="req-context__discussion-author">{{
                discussion.author
              }}</span>
              <span class="req-context__discussion-time">{{
                discussion.created_at
              }}</span>
            </div>
            <p class="req-context__discussion-body">{{ discussion.body }}</p>
          </li>
        </ul>
        <p v-else class="req-context__none">
          {{ t("pm.requirements.discussEmpty") }}
        </p>
      </div>

      <div
        v-else-if="props.activeTab === 'analysis'"
        class="req-context__panel"
      >
        <ul v-if="sortedAnalysis.length > 0" class="req-context__findings">
          <li
            v-for="finding in sortedAnalysis"
            :key="finding.id"
            class="req-context__finding"
          >
            <div class="req-context__finding-head">
              <Badge
                :variant="analysisKindVariant(finding.kind)"
                size="sm"
                :outline="finding.kind !== 'conflict'"
              >
                {{ analysisKindLabel(finding.kind) }}
              </Badge>
              <Badge variant="neutral" size="sm">
                {{
                  t(
                    `pm.requirements.severity${finding.severity.charAt(0).toUpperCase()}${finding.severity.slice(1)}`
                  )
                }}
              </Badge>
            </div>
            <p class="req-context__finding-body">{{ finding.body }}</p>
            <span v-if="finding.related" class="req-context__finding-related">
              {{
                t("pm.requirements.analysisRelated", {
                  target: finding.related
                })
              }}
            </span>
          </li>
        </ul>
        <p v-else class="req-context__none">
          {{ t("pm.requirements.analysisEmpty") }}
        </p>
      </div>

      <div v-else class="req-context__panel">
        <ul v-if="aiSuggestions.length > 0" class="req-context__suggestions">
          <li
            v-for="suggestion in aiSuggestions"
            :key="suggestion.id"
            class="req-context__suggestion"
          >
            <div class="req-context__suggestion-head">
              <Sparkles
                class="req-context__suggestion-icon"
                aria-hidden="true"
              />
              <Badge variant="accent" size="sm" outline>
                {{ t("pm.requirements.aiProposalBadge") }}
              </Badge>
              <span class="req-context__suggestion-time">{{
                suggestion.created_at
              }}</span>
            </div>
            <p class="req-context__suggestion-body">{{ suggestion.body }}</p>
          </li>
        </ul>
        <p v-else class="req-context__none">
          {{ t("pm.requirements.aiEmpty") }}
        </p>
        <p class="req-context__ai-note">
          {{ t("pm.requirements.contextAiNote") }}
        </p>
      </div>
    </template>
  </aside>
</template>

<style scoped>
.req-context {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
  min-height: 0;
}

.req-context__empty {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-8) 0;
}

.req-context__panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  overflow-y: auto;
  min-height: 0;
}

.req-context__none {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}

.req-context__sources,
.req-context__discussions,
.req-context__findings,
.req-context__suggestions {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.req-context__source {
  display: flex;
  gap: var(--space-2);
}

.req-context__source-icon {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  flex: none;
  margin-top: 2px;
  color: var(--text-secondary);
}

.req-context__source-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.req-context__source-kind {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
}

.req-context__source-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.req-context__source-quote {
  margin: 0;
  padding: var(--space-1) var(--space-3);
  border-left: var(--border-2) solid var(--border-default);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.req-context__source-ref {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.req-context__discussion-head {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
}

.req-context__discussion-author {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.req-context__discussion-time {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
}

.req-context__discussion-body {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.req-context__finding-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.req-context__finding-body {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.req-context__finding-related {
  margin-top: var(--space-1);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-context__suggestion {
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  background: var(--ws-color-accent-surface);
}

.req-context__suggestion-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.req-context__suggestion-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--accent);
}

.req-context__suggestion-time {
  margin-left: auto;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.req-context__suggestion-body {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.req-context__ai-note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}
</style>
