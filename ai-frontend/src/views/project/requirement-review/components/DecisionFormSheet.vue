<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Button from "@/components/ui/button/Button.vue";
import Sheet from "@/components/ui/sheet/Sheet.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

/**
 * 决策表单抽屉（S02-UI-03 §5 + Deliverables 5/6）。
 *
 * 双模式：request_changes（要求修改——说明所需修改）/ reject（拒绝——必填拒绝理由）。
 * 文本为必填项：空内容不可提交；提交后由页根落地决策记录。
 * 两个弹层都明确展示决策后果（§6 Clearly communicate the consequences）。
 */
interface Props {
  open: boolean;
  mode: "request_changes" | "reject";
  /** 提案编号（用于标题与后果说明） */
  proposalCode: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "submit", comment: string): void;
}>();

const { t } = useI18n();

const comment = ref("");

watch(
  () => props.open,
  open => {
    if (open) comment.value = "";
  }
);

const title = computed(() =>
  props.mode === "request_changes"
    ? t("pm.requirementReview.changesModalTitle")
    : t("pm.requirementReview.rejectModalTitle")
);

const label = computed(() =>
  props.mode === "request_changes"
    ? t("pm.requirementReview.changesCommentLabel")
    : t("pm.requirementReview.rejectReasonLabel")
);

const placeholder = computed(() =>
  props.mode === "request_changes"
    ? t("pm.requirementReview.changesCommentPlaceholder")
    : t("pm.requirementReview.rejectReasonPlaceholder")
);

const consequence = computed(() =>
  props.mode === "request_changes"
    ? t("pm.requirementReview.changesConsequence")
    : t("pm.requirementReview.rejectConsequence")
);

const canSubmit = computed(() => comment.value.trim().length > 0);

function onSubmit() {
  if (!canSubmit.value) return;
  emit("submit", comment.value.trim());
}
</script>

<template>
  <Sheet
    :open="props.open"
    :title="title"
    :description="props.proposalCode"
    side="right"
    @update:open="emit('update:open', $event)"
  >
    <form class="rrf" @submit.prevent="onSubmit">
      <label class="rrf__field">
        <span class="rrf__label">{{ label }}</span>
        <Textarea
          v-model="comment"
          :placeholder="placeholder"
          :rows="5"
          :invalid="!canSubmit"
        />
      </label>
      <p class="rrf__consequence">{{ consequence }}</p>
      <div class="rrf__footer">
        <Button
          variant="primary"
          size="sm"
          type="submit"
          :disabled="!canSubmit"
        >
          {{ t("common.submit") }}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          type="button"
          @click="emit('update:open', false)"
        >
          {{ t("common.cancel") }}
        </Button>
      </div>
    </form>
  </Sheet>
</template>

<style scoped>
.rrf {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rrf__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.rrf__label {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rrf__consequence {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rrf__footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding-top: var(--space-3);
  border-top: var(--hairline) solid var(--border-default);
}
</style>
