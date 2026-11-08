<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { Construction } from "lucide-vue-next";

import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import AiTeamFrame from "./AiTeamFrame.vue";

/**
 * AI Team 未落地子页占位（Overview / Execution Monitor / Execution Policies）。
 *
 * U-2=A 分步落地：本批只交付 Roles 与 Model Policies 两个内容区；其余三个
 * 子页路由先行注册（页签条可切换），内容区为「即将提供」空态——不做假数据，
 * 与设置域占位页同口径（Settings Framework 原则 P3：不冒充既有能力）。
 */
const { t } = useI18n();
const route = useRoute();

/** 子页描述随路由 meta 切换（meta.aiDesc = 子页名 key；缀「规划中」徽标词） */
const description = computed(() =>
  route.meta.aiDesc
    ? `${t(route.meta.aiDesc as string)} · ${t("settings.common.proposed")}`
    : undefined
);
</script>

<template>
  <AiTeamFrame :description="description">
    <EmptyState
      :title="t('aiTeam.comingSoon.title')"
      :description="t('aiTeam.comingSoon.hint')"
      :icon="Construction"
      size="md"
      data-testid="ai-coming-soon"
    />
  </AiTeamFrame>
</template>
