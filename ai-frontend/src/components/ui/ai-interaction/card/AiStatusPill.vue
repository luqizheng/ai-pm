<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { InteractionStatus } from "../types";

/**
 * 交互状态徽标 —— 11 个生命周期表现态 → 语义色点 + 文案。
 * 只表达「交互进行到哪一步」，不是业务实体状态。
 */
const props = defineProps<{ status: InteractionStatus }>();

const { t } = useI18n();

const TONE: Record<InteractionStatus, StatusIndicatorTone> = {
  created: "neutral",
  presented: "neutral",
  "in-progress": "info",
  submitted: "info",
  validating: "warning",
  processing: "info",
  accepted: "success",
  rejected: "danger",
  deferred: "warning",
  resolved: "success",
  failed: "danger"
};

const LABEL_KEY: Record<InteractionStatus, string> = {
  created: "ai.card.status.created",
  presented: "ai.card.status.presented",
  "in-progress": "ai.card.status.inProgress",
  submitted: "ai.card.status.submitted",
  validating: "ai.card.status.validating",
  processing: "ai.card.status.processing",
  accepted: "ai.card.status.accepted",
  rejected: "ai.card.status.rejected",
  deferred: "ai.card.status.deferred",
  resolved: "ai.card.status.resolved",
  failed: "ai.card.status.failed"
};

const tone = computed(() => TONE[props.status]);
const label = computed(() => t(LABEL_KEY[props.status]));
</script>

<template>
  <StatusIndicator :status="tone" size="sm" :label="label" />
</template>
