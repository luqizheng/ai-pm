<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  BookOpen,
  GitBranch,
  MessageSquareText,
  MessagesSquare,
  Target,
  Compass
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Tabs from "@/components/ui/tabs/Tabs.vue";
import type {
  RequirementEntry,
  RelatedItem,
  SourceItem
} from "../../requirements/requirement-entry";
import { sortDiscussions } from "../../requirements/requirement-entry";

/**
 * 需求详情右栏上下文面板（S02-UI-02 §4）。
 *
 * 四 Tab：Sources / Related Requirements / Discussions / History。
 * 来源条目可打开检查态（emit inspect）；关联需求可导航（emit navigate）；
 * History 为页内修订时间线 + 完整历史入口（emit history，复用 RevisionSheet）。
 */
interface Props {
  entry: RequirementEntry;
  activeTab: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "update:activeTab", value: string): void;
  (e: "inspect", source: SourceItem): void;
  (e: "navigate", item: RelatedItem): void;
  (e: "history"): void;
}>();

const { t } = useI18n();

const tabs = computed(() => [
  { value: "sources", label: t("pm.requirementDetail.tabSources") },
  { value: "related", label: t("pm.requirementDetail.tabRelated") },
  { value: "discussions", label: t("pm.requirementDetail.tabDiscussions") },
  { value: "history", label: t("pm.requirementDetail.tabHistory") }
]);

const sourceIcons: Record<string, Component> = {
  user_request: MessageSquareText,
  discovery: Compass,
  knowledge: BookOpen,
  discussion: MessagesSquare,
  objective: Target
};

const relatedRequirements = computed(() =>
  props.entry.related.filter(item => item.kind === "requirement")
);
const relatedOthers = computed(() =>
  props.entry.related.filter(item => item.kind !== "requirement")
);
const sortedDiscussions = computed(() =>
  sortDiscussions(props.entry.discussions)
);

const sourceKindLabel = (kind: string) =>
  ({
    user_request: t("pm.requirements.sourceUserRequest"),
    discovery: t("pm.requirements.sourceDiscovery"),
    knowledge: t("pm.requirements.sourceKnowledge"),
    discussion: t("pm.requirements.sourceDiscussion"),
    objective: t("pm.requirements.sourceObjective")
  })[kind] ?? kind;

const relatedKindLabel = (kind: string) =>
  ({
    requirement: t("pm.requirements.relatedRequirement"),
    objective: t("pm.requirements.relatedObjective"),
    task: t("pm.requirements.relatedTask")
  })[kind] ?? kind;

const revisionStateLabel = (state: string) =>
  ({
    draft: t("pm.requirements.revStateDraft"),
    pending_confirmation: t("pm.requirements.revStatePending"),
    approved: t("pm.requirements.revStateApproved"),
    rejected: t("pm.requirements.revStateRejected"),
    superseded: t("pm.requirements.revStateSuperseded")
  })[state] ?? state;

const revisionStateVariant = (state: string) =>
  (
    ({
      draft: "neutral",
      pending_confirmation: "warning",
      approved: "success",
      rejected: "danger",
      superseded: "neutral"
    }) as const
  )[state] ?? "neutral";
</script>

<template>
  <aside
    class="reqd-context"
    :aria-label="t('pm.requirementDetail.contextAria')"
  >
    <Tabs
      :model-value="props.activeTab"
      :tabs="tabs"
      :aria-label="t('pm.requirementDetail.contextAria')"
      @update:model-value="emit('update:activeTab', $event)"
    />

    <div v-if="props.activeTab === 'sources'" class="reqd-context__panel">
      <ul v-if="entry.sources.length > 0" class="reqd-context__list">
        <li
          v-for="source in entry.sources"
          :key="source.id"
          class="reqd-context__source"
        >
          <component
            :is="sourceIcons[source.kind]"
            class="reqd-context__source-icon"
            aria-hidden="true"
          />
          <div class="reqd-context__source-body">
            <span class="reqd-context__source-kind">{{
              sourceKindLabel(source.kind)
            }}</span>
            <span class="reqd-context__source-title">{{ source.title }}</span>
            <blockquote class="reqd-context__source-quote">
              {{ source.detail }}
            </blockquote>
            <span v-if="source.ref" class="reqd-context__source-ref">{{
              source.ref
            }}</span>
          </div>
          <button
            type="button"
            class="reqd-context__inspect"
            @click="emit('inspect', source)"
          >
            {{ t("pm.requirementDetail.actionOpenSource") }}
          </button>
        </li>
      </ul>
      <EmptyState v-else size="sm" :title="t('pm.requirements.sourcesEmpty')" />
    </div>

    <div v-else-if="props.activeTab === 'related'" class="reqd-context__panel">
      <template v-if="entry.related.length > 0">
        <div v-if="relatedRequirements.length > 0" class="reqd-context__group">
          <span class="reqd-context__group-title">
            {{ t("pm.requirementDetail.relatedReqsTitle") }}
          </span>
          <button
            v-for="item in relatedRequirements"
            :key="`rel-${item.code}`"
            type="button"
            class="reqd-context__related reqd-context__related--navigable"
            @click="emit('navigate', item)"
          >
            <GitBranch class="reqd-context__source-icon" aria-hidden="true" />
            <span class="reqd-context__item-code">{{ item.code }}</span>
            <span class="reqd-context__item-title">{{ item.title }}</span>
          </button>
        </div>
        <div v-if="relatedOthers.length > 0" class="reqd-context__group">
          <span class="reqd-context__group-title">
            {{ t("pm.requirementDetail.relatedOthersTitle") }}
          </span>
          <div
            v-for="item in relatedOthers"
            :key="`oth-${item.kind}-${item.code}`"
            class="reqd-context__related"
          >
            <span
              class="reqd-context__source-icon reqd-context__source-icon--static"
              aria-hidden="true"
            >
              <Target />
            </span>
            <span class="reqd-context__item-kind">{{
              relatedKindLabel(item.kind)
            }}</span>
            <span class="reqd-context__item-code">{{ item.code }}</span>
            <span class="reqd-context__item-title">{{ item.title }}</span>
          </div>
        </div>
      </template>
      <p v-else class="reqd-context__none">
        {{ t("pm.requirements.relatedEmpty") }}
      </p>
    </div>

    <div
      v-else-if="props.activeTab === 'discussions'"
      class="reqd-context__panel"
    >
      <ul v-if="sortedDiscussions.length > 0" class="reqd-context__list">
        <li v-for="discussion in sortedDiscussions" :key="discussion.id">
          <div class="reqd-context__discussion-head">
            <span class="reqd-context__discussion-author">{{
              discussion.author
            }}</span>
            <span class="reqd-context__discussion-time">{{
              discussion.created_at
            }}</span>
          </div>
          <p class="reqd-context__discussion-body">{{ discussion.body }}</p>
        </li>
      </ul>
      <p v-else class="reqd-context__none">
        {{ t("pm.requirements.discussEmpty") }}
      </p>
    </div>

    <div v-else class="reqd-context__panel">
      <ul class="reqd-context__list reqd-context__history">
        <li
          v-for="revision in entry.revisions"
          :key="revision.id"
          class="reqd-context__revision"
        >
          <div class="reqd-context__revision-head">
            <span class="reqd-context__revision-version">
              {{
                t("pm.requirements.revisionVersion", {
                  version: revision.version
                })
              }}
            </span>
            <Badge :variant="revisionStateVariant(revision.state)" size="sm">
              {{ revisionStateLabel(revision.state) }}
            </Badge>
          </div>
          <span class="reqd-context__revision-summary">{{
            revision.summary
          }}</span>
          <span
            v-if="revision.decision"
            class="reqd-context__revision-decision"
          >
            {{
              revision.decision.kind === "confirmed"
                ? t("pm.requirementHistory.decisionConfirmed", {
                    actor: revision.decision.actor,
                    time: revision.decision.acted_at
                  })
                : t("pm.requirementHistory.decisionRejected", {
                    actor: revision.decision.actor,
                    time: revision.decision.acted_at
                  })
            }}
          </span>
          <span class="reqd-context__revision-meta">
            {{ revision.author }} · {{ revision.created_at }}
          </span>
        </li>
      </ul>
      <Button variant="secondary" size="sm" @click="emit('history')">
        {{ t("pm.requirementDetail.historyOpenFull") }}
      </Button>
    </div>
  </aside>
</template>

<style scoped>
.reqd-context {
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

.reqd-context__panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  overflow-y: auto;
  min-height: 0;
}

.reqd-context__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.reqd-context__source {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
}

.reqd-context__source-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  margin-top: 2px;
  color: var(--text-secondary);
}

.reqd-context__source-icon--static {
  display: inline-flex;
}

.reqd-context__source-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  flex: 1 1 0;
}

.reqd-context__source-kind {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
}

.reqd-context__source-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.reqd-context__source-quote {
  margin: 0;
  padding: var(--space-1) var(--space-3);
  border-left: var(--border-2) solid var(--border-default);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.reqd-context__source-ref {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.reqd-context__inspect {
  flex: none;
  align-self: center;
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
  cursor: pointer;
  white-space: nowrap;
}

.reqd-context__inspect:hover {
  color: var(--accent-hover);
}

.reqd-context__inspect:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.reqd-context__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.reqd-context__group-title {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.reqd-context__related {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: transparent;
  padding: var(--space-2) var(--space-3);
  font-family: var(--font-ui);
  text-align: left;
}

button.reqd-context__related {
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

button.reqd-context__related:hover {
  background: var(--ws-hover-veil);
  border-color: var(--accent);
}

button.reqd-context__related:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.reqd-context__related--navigable {
  border-color: var(--accent);
}

.reqd-context__item-kind {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  flex: none;
}

.reqd-context__item-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  flex: none;
}

.reqd-context__item-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.reqd-context__discussion-head {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
}

.reqd-context__discussion-author {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.reqd-context__discussion-time {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
}

.reqd-context__discussion-body {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.reqd-context__history {
  gap: var(--space-3);
}

.reqd-context__revision {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  border-bottom: var(--hairline) solid var(--border-default);
  padding-bottom: var(--space-2);
}

.reqd-context__revision:last-child {
  border-bottom: 0;
  padding-bottom: 0;
}

.reqd-context__revision-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.reqd-context__revision-version {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.reqd-context__revision-summary {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.reqd-context__revision-meta {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.reqd-context__revision-decision {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-primary);
}

.reqd-context__none {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}
</style>
