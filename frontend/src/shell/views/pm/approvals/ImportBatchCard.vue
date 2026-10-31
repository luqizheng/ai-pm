<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useToast } from "@/components/ui/toast";
import { getImportProposals } from "@/api/imports";
import { decideApproval } from "@/api/pm";
import type { ProposalRef } from "@/api/types";

const props = defineProps<{ projectId: string; importId: string }>();

const { t } = useI18n();
const toast = useToast();

const proposals = ref<ProposalRef[]>([]);
const loading = ref(false);
const busy = ref(false);
const failedKinds = ref<string[]>([]);

async function load() {
  loading.value = true;
  try {
    const proposed = await getImportProposals(props.projectId, props.importId);
    proposals.value = proposed.proposals ?? [];
  } catch {
    proposals.value = [];
  } finally {
    loading.value = false;
  }
}

async function decideAll(decision: "approved" | "rejected") {
  if (busy.value) return;
  busy.value = true;
  failedKinds.value = [];
  for (const item of proposals.value) {
    try {
      await decideApproval(item.approval_id, {
        decision,
        reason: t("pm.importBatch.reason")
      });
    } catch {
      failedKinds.value.push(item.kind);
    }
  }
  busy.value = false;
  if (failedKinds.value.length === 0) {
    toast.toast({ variant: "success", title: t("pm.importBatch.done") });
  }
}

onMounted(load);
</script>

<template>
  <section v-if="loading || proposals.length" class="batch">
    <header class="batch__head">
      <div>
        <h3 class="batch__title">
          {{ t("pm.importBatch.title", { count: proposals.length }) }}
        </h3>
        <p class="batch__hint">{{ importId }}</p>
      </div>
      <div class="batch__actions">
        <Button
          size="sm"
          variant="secondary"
          :disabled="busy"
          @click="decideAll('rejected')"
        >
          {{ t("pm.importBatch.rejectAll") }}
        </Button>
        <Button size="sm" :loading="busy" @click="decideAll('approved')">
          {{ t("pm.importBatch.approveAll") }}
        </Button>
      </div>
    </header>

    <ul class="batch__items">
      <li v-for="item in proposals" :key="item.proposal_id" class="batch__item">
        <span>{{ item.kind }}</span>
        <Badge :variant="failedKinds.includes(item.kind) ? 'danger' : 'neutral'">
          {{ item.item_count }}
        </Badge>
      </li>
    </ul>

    <p v-if="failedKinds.length" class="batch__failed">
      {{ t("pm.importBatch.partial", { count: failedKinds.length }) }}
    </p>
  </section>
</template>

<style scoped>
.batch {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--border-accent-glow);
  border-radius: var(--radius-md);
  background: var(--accent-subtle);
}

.batch__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.batch__title {
  margin: 0;
  font-size: var(--font-size-h4);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.batch__hint {
  margin: var(--space-1) 0 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.batch__actions {
  display: flex;
  gap: var(--space-2);
  flex: none;
}

.batch__items {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.batch__item {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.batch__failed {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--danger-strong);
}
</style>