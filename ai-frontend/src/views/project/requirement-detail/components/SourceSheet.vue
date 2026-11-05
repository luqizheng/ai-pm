<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  BookOpen,
  Compass,
  MessageSquareText,
  MessagesSquare,
  Target
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Sheet from "@/components/ui/sheet/Sheet.vue";
import type { SourceItem } from "../../requirements/requirement-entry";

/**
 * 来源检查抽屉（S02-UI-02 §5 View source / Open evidence + Deliverable 3）。
 *
 * 展示单条来源的完整信息（类型 / 原文摘录 / 外部引用）。
 * 红线呈现：来源材料是证据，不是正式需求——面板内明确标注。
 */
interface Props {
  open: boolean;
  source: SourceItem | null;
  entryCode: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{ (e: "update:open", value: boolean): void }>();

const { t } = useI18n();

const sourceIcons: Record<string, Component> = {
  user_request: MessageSquareText,
  discovery: Compass,
  knowledge: BookOpen,
  discussion: MessagesSquare,
  objective: Target
};

const sourceKindLabel = computed(() =>
  props.source
    ? ({
        user_request: t("pm.requirements.sourceUserRequest"),
        discovery: t("pm.requirements.sourceDiscovery"),
        knowledge: t("pm.requirements.sourceKnowledge"),
        discussion: t("pm.requirements.sourceDiscussion"),
        objective: t("pm.requirements.sourceObjective")
      }[props.source.kind] ?? props.source.kind)
    : ""
);
</script>

<template>
  <Sheet
    :open="props.open"
    :title="t('pm.requirementDetail.sourceSheetTitle')"
    :description="props.entryCode"
    side="right"
    @update:open="emit('update:open', $event)"
  >
    <div v-if="props.source" class="reqd-source">
      <div class="reqd-source__head">
        <component
          :is="sourceIcons[props.source.kind]"
          class="reqd-source__icon"
          aria-hidden="true"
        />
        <Badge variant="neutral" size="sm">{{ sourceKindLabel }}</Badge>
        <span v-if="props.source.ref" class="reqd-source__ref">{{
          props.source.ref
        }}</span>
      </div>

      <h4 class="reqd-source__title">{{ props.source.title }}</h4>

      <blockquote class="reqd-source__quote">
        {{ props.source.detail }}
      </blockquote>

      <p class="reqd-source__note">
        {{ t("pm.requirementDetail.sourceSheetNote") }}
      </p>

      <div class="reqd-source__footer">
        <Button
          variant="secondary"
          size="sm"
          @click="emit('update:open', false)"
        >
          {{ t("pm.requirements.historyClose") }}
        </Button>
      </div>
    </div>
  </Sheet>
</template>

<style scoped>
.reqd-source {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.reqd-source__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.reqd-source__icon {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  color: var(--text-secondary);
}

.reqd-source__ref {
  margin-left: auto;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.reqd-source__title {
  margin: 0;
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.reqd-source__quote {
  margin: 0;
  padding: var(--space-3);
  border-left: var(--border-2) solid var(--border-default);
  border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  background: var(--surface-raised);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.reqd-source__note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.reqd-source__footer {
  display: flex;
  justify-content: flex-end;
}
</style>
