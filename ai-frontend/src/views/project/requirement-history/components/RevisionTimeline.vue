<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  Inbox,
  ShieldCheck,
  Sparkles,
  UserRound,
  XCircle
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import type {
  RevisionOrigin,
  RevisionRecord
} from "../../requirements/requirement-entry";

/**
 * 垂直修订时间线（S02-UI-05 §3 + Deliverable 1）。
 *
 * 每条目：修订号 / 作者 / 时间戳 / 变更摘要 / 变更来源 / 关联决策；
 * 四类可辨：人工编辑（neutral）、AI 辅助提案（accent outline）、
 * 已确认修订（success 圆点 + 决策徽标）、被拒绝提案（danger 圆点 + 决策徽标）。
 */
interface Props {
  revisions: RevisionRecord[];
  selectedId: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{ (e: "select", id: string): void }>();

const { t } = useI18n();

const originMeta: Record<RevisionOrigin, { icon: Component; key: string }> = {
  human_edit: { icon: UserRound, key: "originHuman" },
  ai_assisted: { icon: Sparkles, key: "originAi" },
  import: { icon: Inbox, key: "originImport" }
};

const stateTone = (state: string): StatusIndicatorTone =>
  (
    ({
      draft: "neutral",
      pending_confirmation: "warning",
      approved: "success",
      rejected: "danger",
      superseded: "neutral"
    }) as const
  )[state] ?? "neutral";

const stateLabel = (state: string) =>
  ({
    draft: t("pm.requirements.revStateDraft"),
    pending_confirmation: t("pm.requirements.revStatePending"),
    approved: t("pm.requirements.revStateApproved"),
    rejected: t("pm.requirements.revStateRejected"),
    superseded: t("pm.requirements.revStateSuperseded")
  })[state] ?? state;

const decisionMeta = computed(() => ({
  confirmed: { icon: ShieldCheck as Component, key: "decisionConfirmed" },
  rejected: { icon: XCircle as Component, key: "decisionRejected" }
}));

const ordered = computed(() => props.revisions);
</script>

<template>
  <section
    class="rh-timeline"
    :aria-label="t('pm.requirementHistory.timelineAria')"
  >
    <h3 class="rh-timeline__title">
      {{ t("pm.requirementHistory.timelineTitle") }}
    </h3>
    <EmptyState
      v-if="ordered.length === 0"
      size="sm"
      :title="t('pm.requirementHistory.timelineEmpty')"
    />
    <ol v-else class="rh-timeline__list">
      <li v-for="revision in ordered" :key="revision.id">
        <button
          type="button"
          class="rh-timeline__item"
          :class="{
            'rh-timeline__item--selected': revision.id === props.selectedId
          }"
          :aria-current="revision.id === props.selectedId ? 'true' : undefined"
          @click="emit('select', revision.id)"
        >
          <span class="rh-timeline__node" aria-hidden="true">
            <StatusIndicator :status="stateTone(revision.state)" size="sm" />
          </span>
          <span class="rh-timeline__body">
            <span class="rh-timeline__head">
              <span class="rh-timeline__version">
                {{
                  t("pm.requirements.revisionVersion", {
                    version: revision.version
                  })
                }}
              </span>
              <Badge variant="neutral" size="sm">
                {{ stateLabel(revision.state) }}
              </Badge>
              <span
                v-if="revision.origin"
                class="rh-timeline__origin"
                :class="{
                  'rh-timeline__origin--ai': revision.origin === 'ai_assisted'
                }"
              >
                <component
                  :is="originMeta[revision.origin].icon"
                  aria-hidden="true"
                />
                <span>{{
                  t(`pm.requirementHistory.${originMeta[revision.origin].key}`)
                }}</span>
              </span>
            </span>
            <span class="rh-timeline__summary">{{ revision.summary }}</span>
            <span class="rh-timeline__meta">
              <span>{{ revision.author }}</span>
              <span class="rh-timeline__time">{{ revision.created_at }}</span>
            </span>
            <span
              v-if="revision.decision"
              class="rh-timeline__decision"
              :class="`rh-timeline__decision--${revision.decision.kind}`"
            >
              <component
                :is="decisionMeta[revision.decision.kind].icon"
                aria-hidden="true"
              />
              <span>
                {{
                  t(
                    `pm.requirementHistory.${decisionMeta[revision.decision.kind].key}`,
                    {
                      actor: revision.decision.actor,
                      time: revision.decision.acted_at
                    }
                  )
                }}
              </span>
            </span>
          </span>
        </button>
      </li>
    </ol>
  </section>
</template>

<style scoped>
.rh-timeline {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rh-timeline__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rh-timeline__list {
  display: flex;
  flex-direction: column;
  gap: 0;
  margin: 0;
  padding: 0;
  list-style: none;
  position: relative;
}

/* 垂直时间线竖线 */
.rh-timeline__list::before {
  content: "";
  position: absolute;
  left: 7px;
  top: var(--space-4);
  bottom: var(--space-4);
  width: var(--border-1);
  background: var(--border-default);
}

.rh-timeline__item {
  display: flex;
  gap: var(--space-3);
  width: 100%;
  padding: var(--space-3) var(--space-2);
  border: 0;
  border-left: var(--border-2) solid transparent;
  background: transparent;
  font-family: var(--font-ui);
  text-align: left;
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-standard);
}

.rh-timeline__item:hover {
  background: var(--ws-hover-veil);
}

.rh-timeline__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.rh-timeline__item--selected {
  background: var(--ws-color-accent-surface);
  border-left-color: var(--accent);
}

.rh-timeline__node {
  flex: none;
  display: inline-flex;
  align-items: flex-start;
  padding-top: 2px;
  position: relative;
  z-index: 1;
}

.rh-timeline__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.rh-timeline__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rh-timeline__version {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rh-timeline__origin {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rh-timeline__origin svg {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
}

.rh-timeline__origin--ai {
  color: var(--accent);
}

.rh-timeline__summary {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rh-timeline__meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rh-timeline__time {
  color: var(--text-disabled);
  font-variant-numeric: tabular-nums;
}

.rh-timeline__decision {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.rh-timeline__decision svg {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
}

.rh-timeline__decision--confirmed {
  color: var(--success-strong);
}

.rh-timeline__decision--rejected {
  color: var(--danger-strong);
}
</style>
