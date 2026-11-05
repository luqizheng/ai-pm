<script setup lang="ts">
import AiSubmissionStatus from "../card/AiSubmissionStatus.vue";
import type { SubmissionPhase, SubmitResult } from "../types";

/**
 * Control-family facade over the card submission bar, so interaction controls
 * share one submit / success / failure presentation (and one retry channel).
 */
withDefaults(
  defineProps<{
    phase: SubmissionPhase;
    result?: SubmitResult | null;
    errorMessage?: string | null;
  }>(),
  {
    result: null,
    errorMessage: null
  }
);

const emit = defineEmits<{ retry: [] }>();
</script>

<template>
  <div class="ai-control-submission">
    <AiSubmissionStatus
      :phase="phase"
      :result="result"
      :error-message="errorMessage"
      @retry="emit('retry')"
    />
  </div>
</template>

<style scoped>
.ai-control-submission {
  display: block;
  min-width: 0;
}
</style>
