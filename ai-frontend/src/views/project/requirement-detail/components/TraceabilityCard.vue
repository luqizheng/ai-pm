<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  ArrowRight,
  BookOpen,
  Compass,
  GitBranch,
  MessageSquareText,
  MessagesSquare,
  Sparkles,
  Target
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import type {
  RequirementEntry,
  RelatedItem,
  SourceItem
} from "../../requirements/requirement-entry";
import { currentRevision } from "../../requirements/requirement-entry";

/**
 * 追溯链卡（S02-UI-02 §3D + Deliverable 2 展开追溯视图）。
 *
 * 上游来源（原始请求 / 发现证据 / 知识来源 / 讨论 / 目标）→ 正式需求（中心节点）
 * → 下游关联（关联需求 / 目标 / 任务）。
 * 原始来源材料用引用块视觉，与正式需求节点明确区分；默认收起（每段最多
 * 2 条 + 「+N」计数），展开后显示全部并开放逐条操作（查看来源 / 导航）。
 */
interface Props {
  entry: RequirementEntry;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "inspect", source: SourceItem): void;
  (e: "navigate", item: RelatedItem): void;
}>();

const { t } = useI18n();

const COLLAPSED_LIMIT = 2;
const expanded = ref(false);

const sourceIcons: Record<string, Component> = {
  user_request: MessageSquareText,
  discovery: Compass,
  knowledge: BookOpen,
  discussion: MessagesSquare,
  objective: Target
};

const currentVersion = computed(
  () => currentRevision(props.entry)?.version ?? 1
);

const upstreamAll = computed(() => props.entry.sources);
const downstreamAll = computed(() => props.entry.related);

const upstreamVisible = computed(() =>
  expanded.value
    ? upstreamAll.value
    : upstreamAll.value.slice(0, COLLAPSED_LIMIT)
);
const downstreamVisible = computed(() =>
  expanded.value
    ? downstreamAll.value
    : downstreamAll.value.slice(0, COLLAPSED_LIMIT)
);

const upstreamHidden = computed(() =>
  Math.max(0, upstreamAll.value.length - upstreamVisible.value.length)
);
const downstreamHidden = computed(() =>
  Math.max(0, downstreamAll.value.length - downstreamVisible.value.length)
);

const hasUpstreamMore = computed(
  () => upstreamAll.value.length > COLLAPSED_LIMIT
);
const hasDownstreamMore = computed(
  () => downstreamAll.value.length > COLLAPSED_LIMIT
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
</script>

<template>
  <section class="reqd-card" :aria-label="t('pm.requirementDetail.traceTitle')">
    <div class="reqd-trace__head">
      <h3 class="reqd-card__title">
        {{ t("pm.requirementDetail.traceTitle") }}
      </h3>
      <Button variant="ghost" size="sm" @click="expanded = !expanded">
        {{
          expanded
            ? t("pm.requirementDetail.traceCollapse")
            : t("pm.requirementDetail.traceExpand")
        }}
      </Button>
    </div>

    <div class="reqd-trace__chain">
      <div
        class="reqd-trace__col"
        :aria-label="t('pm.requirementDetail.traceUpstream')"
      >
        <span class="reqd-trace__col-title">{{
          t("pm.requirementDetail.traceUpstream")
        }}</span>
        <button
          v-for="source in upstreamVisible"
          :key="source.id"
          type="button"
          class="reqd-trace__item reqd-trace__item--source"
          @click="emit('inspect', source)"
        >
          <component
            :is="sourceIcons[source.kind]"
            class="reqd-trace__item-icon"
            aria-hidden="true"
          />
          <span class="reqd-trace__item-body">
            <span class="reqd-trace__item-kind">{{
              sourceKindLabel(source.kind)
            }}</span>
            <span class="reqd-trace__item-title">{{ source.title }}</span>
            <span v-if="source.ref" class="reqd-trace__item-ref">{{
              source.ref
            }}</span>
          </span>
        </button>
        <button
          v-if="hasUpstreamMore"
          type="button"
          class="reqd-trace__more"
          @click="expanded = true"
        >
          {{ t("pm.requirementDetail.traceHidden", { count: upstreamHidden }) }}
        </button>
      </div>

      <div class="reqd-trace__arrow" aria-hidden="true">
        <ArrowRight />
      </div>

      <div
        class="reqd-trace__col reqd-trace__col--center"
        :aria-label="t('pm.requirementDetail.traceRequirement')"
      >
        <span class="reqd-trace__col-title">{{
          t("pm.requirementDetail.traceRequirement")
        }}</span>
        <div class="reqd-trace__node">
          <Sparkles class="reqd-trace__node-icon" aria-hidden="true" />
          <span class="reqd-trace__node-code">{{ entry.code }}</span>
          <span class="reqd-trace__node-title">{{ entry.title }}</span>
          <Badge variant="accent" size="sm">
            {{
              t("pm.requirementDetail.currentVersion", {
                version: currentVersion
              })
            }}
          </Badge>
        </div>
      </div>

      <div class="reqd-trace__arrow" aria-hidden="true">
        <ArrowRight />
      </div>

      <div
        class="reqd-trace__col"
        :aria-label="t('pm.requirementDetail.traceDownstream')"
      >
        <span class="reqd-trace__col-title">{{
          t("pm.requirementDetail.traceDownstream")
        }}</span>
        <button
          v-for="item in downstreamVisible"
          :key="`${item.kind}-${item.code}`"
          type="button"
          class="reqd-trace__item"
          :class="{
            'reqd-trace__item--navigable':
              item.kind === 'requirement' && item.target_id
          }"
          :disabled="!(item.kind === 'requirement' && item.target_id)"
          @click="
            item.kind === 'requirement' &&
            item.target_id &&
            emit('navigate', item)
          "
        >
          <GitBranch class="reqd-trace__item-icon" aria-hidden="true" />
          <span class="reqd-trace__item-body">
            <span class="reqd-trace__item-kind">{{
              relatedKindLabel(item.kind)
            }}</span>
            <span class="reqd-trace__item-title">
              <span class="reqd-trace__item-code">{{ item.code }}</span>
              {{ item.title }}
            </span>
          </span>
        </button>
        <button
          v-if="hasDownstreamMore"
          type="button"
          class="reqd-trace__more"
          @click="expanded = true"
        >
          {{
            t("pm.requirementDetail.traceHidden", { count: downstreamHidden })
          }}
        </button>
      </div>
    </div>

    <p class="reqd-trace__note">{{ t("pm.requirementDetail.traceNote") }}</p>
  </section>
</template>

<style scoped>
.reqd-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.reqd-card__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.reqd-trace__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.reqd-trace__chain {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) auto minmax(0, 1fr);
  gap: var(--space-2);
  align-items: start;
}

.reqd-trace__col {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.reqd-trace__col-title {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.reqd-trace__arrow {
  display: flex;
  align-items: center;
  color: var(--text-disabled);
  margin-top: var(--space-6);
}

.reqd-trace__arrow svg {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
}

.reqd-trace__item {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  width: 100%;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: transparent;
  padding: var(--space-2) var(--space-3);
  font-family: var(--font-ui);
  text-align: left;
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

.reqd-trace__item:hover:not(:disabled) {
  background: var(--ws-hover-veil);
  border-color: var(--border-hover-strong);
}

.reqd-trace__item:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.reqd-trace__item:disabled {
  cursor: default;
}

.reqd-trace__item--navigable {
  border-color: var(--accent);
}

.reqd-trace__item--navigable:hover {
  border-color: var(--accent-hover);
}

.reqd-trace__item-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  margin-top: 2px;
  color: var(--text-secondary);
}

.reqd-trace__item-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.reqd-trace__item-kind {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.reqd-trace__item-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reqd-trace__item-code {
  font-family: var(--font-mono);
  margin-right: var(--space-1);
}

.reqd-trace__item-ref {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.reqd-trace__more {
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--accent);
  text-align: left;
  cursor: pointer;
}

.reqd-trace__more:hover {
  color: var(--accent-hover);
}

.reqd-trace__more:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.reqd-trace__node {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-sm);
  background: var(--ws-color-accent-surface);
  padding: var(--space-3);
}

.reqd-trace__node-icon {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  color: var(--accent);
}

.reqd-trace__node-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.reqd-trace__node-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.reqd-trace__note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

@media (max-width: 1100px) {
  .reqd-trace__chain {
    grid-template-columns: minmax(0, 1fr);
  }

  .reqd-trace__arrow {
    transform: rotate(90deg);
    margin-top: 0;
    justify-content: center;
  }
}
</style>
