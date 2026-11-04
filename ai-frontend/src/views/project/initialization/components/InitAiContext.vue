<script setup lang="ts">
import { computed, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { LoaderCircle, Send, Sparkles } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import type { BadgeVariant } from "@/components/ui/badge";
import type {
  AiContextItem,
  AiContextSectionId,
  FactLayer
} from "@/api/initialization-overview";

/**
 * AI Context Panel —— 初始化阶段的 AI 工作上下文（设计稿 §7）。
 *
 * 它是**只读上下文**，不是第二个聊天窗口（设计系统 §二 布局规则 1）：
 *   - 无输入框、无对话、无「一键确认」；
 *   - 五类信息：执行中 / 最近完成 / 待确认提案 / 当前问题 / 下一步。
 *
 * 事实层级（沿用 S01-05 §D-b）：
 *   - 实心 = 已成立的正式事实；
 *   - 描边 accent = AI 产出、待人工确认；
 *   - **AI 事件一律不用对勾**——circle-check 只允许出现在「人类确认」上。
 */

interface Props {
  items: AiContextItem[];
}

const props = defineProps<Props>();

const { t } = useI18n();

const SECTION_ORDER: AiContextSectionId[] = [
  "running",
  "recent",
  "proposals",
  "issues",
  "next"
];

const LAYER_META: Record<
  FactLayer,
  { variant: BadgeVariant; icon: Component; labelKey: string }
> = {
  fact: { variant: "neutral", icon: Send, labelKey: "layerFact" },
  proposal: { variant: "accent", icon: Sparkles, labelKey: "layerProposal" },
  running: { variant: "neutral", icon: LoaderCircle, labelKey: "layerRunning" }
};

/** 条目标题：阶段派生条目用 i18n key，后端数据条目用原文 */
const itemTitle = (item: AiContextItem) =>
  item.titleKey
    ? t(`s01.initialization-overview.${item.titleKey}.title`)
    : item.title;

/**
 * 状态标识。
 * - 「最近完成」分区是后端 activity_type（snake_case），走本地化词表；
 * - 其余分区的状态是领域原始值（finding 状态、风险级别、阶段状态），
 *   按原样展示——不做无依据的翻译。
 */
const itemState = (item: AiContextItem) => {
  if (!item.state) return "";
  if (item.section === "recent") {
    return t(
      `s01.initialization-overview.aiContext.activity.${item.state}`,
      item.state
    );
  }
  if (item.section === "running" || item.section === "next") {
    return t(`s01.initialization-overview.status.${item.state}`, item.state);
  }
  return item.state;
};

const sections = computed(() =>
  SECTION_ORDER.map(id => ({
    id,
    title: t(`s01.initialization-overview.aiContext.sections.${id}`),
    items: props.items.filter(item => item.section === id)
  })).filter(section => section.items.length > 0)
);
</script>

<template>
  <aside
    class="init-ai-context"
    :aria-label="t('s01.initialization-overview.aiContext.title')"
    data-testid="init-ai-context"
  >
    <header class="init-ai-context__header">
      <h2 class="init-ai-context__title">
        {{ t("s01.initialization-overview.aiContext.title") }}
      </h2>
      <Badge variant="neutral" size="sm">
        {{ t("s01.initialization-overview.aiContext.readOnly") }}
      </Badge>
    </header>

    <p v-if="sections.length === 0" class="init-ai-context__empty">
      {{ t("s01.initialization-overview.aiContext.empty") }}
    </p>

    <div v-else class="init-ai-context__body">
      <section
        v-for="section in sections"
        :key="section.id"
        class="init-ai-context__section"
      >
        <h3 class="init-ai-context__heading">{{ section.title }}</h3>
        <ul class="init-ai-context__list">
          <li
            v-for="item in section.items"
            :key="item.id"
            class="init-ai-context__item"
            :class="`is-${item.layer}`"
          >
            <div class="init-ai-context__item-head">
              <Badge :variant="LAYER_META[item.layer].variant" size="sm">
                <component
                  :is="LAYER_META[item.layer].icon"
                  :size="12"
                  aria-hidden="true"
                />
                <span>
                  {{
                    t(
                      `s01.initialization-overview.aiContext.${LAYER_META[item.layer].labelKey}`
                    )
                  }}
                </span>
              </Badge>
              <span v-if="item.state" class="init-ai-context__state">
                {{ itemState(item) }}
              </span>
            </div>
            <p class="init-ai-context__item-title">{{ itemTitle(item) }}</p>
            <p v-if="item.meta" class="init-ai-context__item-meta">
              {{ item.meta }}
            </p>
          </li>
        </ul>
      </section>
    </div>
  </aside>
</template>

<style scoped>
.init-ai-context {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4);
  background: var(--ws-sidebar);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.init-ai-context__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.init-ai-context__title {
  flex: 1;
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
}

.init-ai-context__empty {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.init-ai-context__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.init-ai-context__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.init-ai-context__heading {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
}

.init-ai-context__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.init-ai-context__item {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

/* 描边 = AI 产出待确认；实心 = 已成立的正式事实 */
.init-ai-context__item.is-proposal {
  border-color: var(--ws-accent);
}

.init-ai-context__item-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.init-ai-context__state {
  overflow: hidden;
  color: var(--ws-text-disabled);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.init-ai-context__item-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

.init-ai-context__item-meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}
</style>
