<script setup lang="ts">
import { computed, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowRight, Cog, Sparkles, UserRound } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import type { BadgeVariant } from "@/components/ui/badge";
import type { ActionOwner, CurrentAction } from "@/api/initialization-overview";
import InitStatusBadge from "./InitStatusBadge.vue";

/**
 * Current Action —— 当前最重要的一件事（设计稿 §5）。
 *
 * 页面首屏第一视觉块。它只回答一件事：**现在谁在做什么，我要不要动**。
 *
 * 纪律：
 *   - 责任主体必须显式（Human / AI Agent / System），AI 主体必须附权威来源；
 *   - 「需要你操作」与「无需你操作」措辞相反且不可省略；
 *   - 其余同类事项只给数量（「另有 N 项」），不铺开成列表。
 */

interface Props {
  action: CurrentAction;
}

const props = defineProps<Props>();

const { t } = useI18n();

const OWNER_META: Record<
  ActionOwner,
  { variant: BadgeVariant; icon: Component; labelKey: string }
> = {
  human: { variant: "warning", icon: UserRound, labelKey: "ownerHuman" },
  ai: { variant: "accent", icon: Sparkles, labelKey: "ownerAi" },
  system: { variant: "neutral", icon: Cog, labelKey: "ownerSystem" }
};

const ownerMeta = computed(() => OWNER_META[props.action.owner]);

const stageTitle = computed(() =>
  t(`s01.initialization-overview.${props.action.titleKey}.title`)
);

const actionTitle = computed(() =>
  t(`s01.initialization-overview.current.actionTitle.${props.action.owner}`, {
    stage: stageTitle.value
  })
);

const sourceText = computed(() => props.action.sourceState ?? "");

/** 实况：结构化合成优先，其次后端原文 */
const detailText = computed(() => {
  if (props.action.detailKey) {
    return t(
      `s01.initialization-overview.${props.action.detailKey}`,
      props.action.detailParams ?? {}
    );
  }
  return props.action.detail ?? "";
});
</script>

<template>
  <section class="init-current" data-testid="init-current-action">
    <header class="init-current__header">
      <h2 class="init-current__heading">
        {{ t("s01.initialization-overview.current.title") }}
      </h2>
      <Badge :variant="ownerMeta.variant" size="sm">
        <component :is="ownerMeta.icon" :size="12" aria-hidden="true" />
        <span>{{
          t(`s01.initialization-overview.current.${ownerMeta.labelKey}`)
        }}</span>
      </Badge>
      <InitStatusBadge :status="props.action.status" size="sm" />
    </header>

    <p class="init-current__title">{{ actionTitle }}</p>

    <p v-if="detailText || sourceText" class="init-current__detail">
      <span v-if="detailText">{{ detailText }}</span>
      <code v-if="sourceText" class="init-current__source">{{
        sourceText
      }}</code>
    </p>

    <footer class="init-current__footer">
      <span
        class="init-current__ask"
        :class="{ 'is-user': props.action.needsUser }"
      >
        {{
          props.action.needsUser
            ? t("s01.initialization-overview.current.needsUser")
            : t("s01.initialization-overview.current.noUserAction")
        }}
      </span>
      <span v-if="props.action.remaining > 0" class="init-current__remaining">
        {{
          t("s01.initialization-overview.current.remaining", {
            count: props.action.remaining
          })
        }}
      </span>
      <Button
        v-if="props.action.route"
        variant="secondary"
        size="sm"
        @click="$emit('open')"
      >
        {{ t("s01.initialization-overview.current.open") }}
        <ArrowRight :size="14" aria-hidden="true" />
      </Button>
    </footer>
  </section>
</template>

<style scoped>
.init-current {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.init-current__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.init-current__heading {
  flex: 1;
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
}

.init-current__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.init-current__detail {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

.init-current__source {
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.init-current__footer {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding-top: var(--space-3);
  border-top: var(--border-1) solid var(--ws-border);
}

.init-current__ask {
  flex: 1;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.init-current__ask.is-user {
  color: var(--ws-color-warning-text);
}

.init-current__remaining {
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
}
</style>
