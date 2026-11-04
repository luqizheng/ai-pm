<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { CircleCheck, LoaderCircle } from "lucide-vue-next";

import { StatusIndicator } from "@/components/ui/status-indicator";
import { ProgressIndicator } from "@/components/ui/progress-indicator";
import { EmptyState } from "@/components/ui/empty-state";
import { Badge } from "@/components/ui/badge";
import { Alert } from "@/components/ui/alert";
import { Toast } from "@/components/ui/toast";
import { Skeleton } from "@/components/ui/skeleton";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/** /design/components/feedback —— DS/Feedback 族样板 */
const { t } = useI18n();

const tones = [
  "neutral",
  "info",
  "success",
  "warning",
  "danger",
  "accent"
] as const;
const sizes = ["sm", "md", "lg"] as const;
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.components.feedback.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.components.feedback.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>StatusIndicator (13:34203)</h2>
      <div class="design-page__row">
        <Specimen v-for="tone in tones" :key="tone" :label="`Status=${tone}`">
          <StatusIndicator :status="tone" :label="tone" />
        </Specimen>
        <Specimen label="Dot only">
          <StatusIndicator status="success" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>ProgressIndicator (13:34246)</h2>
      <div class="design-page__row">
        <Specimen
          v-for="s in sizes"
          :key="s"
          :label="`Size=${s}`"
          width="280px"
        >
          <ProgressIndicator :value="60" :size="s" />
        </Specimen>
        <Specimen label="Indeterminate" width="280px">
          <ProgressIndicator indeterminate />
        </Specimen>
        <Specimen label="Variant=danger" width="280px">
          <ProgressIndicator
            :value="90"
            variant="danger"
            label="Quota almost full"
          />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>EmptyState (13:34322)</h2>
      <div class="design-page__row">
        <Specimen label="Size=md · Actions" width="320px">
          <EmptyState
            title="No repositories yet"
            description="Connect a repository to begin code discovery."
            action-label="Connect"
            secondary-action-label="Docs"
          />
        </Specimen>
        <Specimen label="Size=sm · Custom icon" width="280px">
          <EmptyState title="All caught up" :icon="CircleCheck" />
        </Specimen>
        <Specimen label="Size=lg" width="360px">
          <EmptyState title="Nothing here yet" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>Badge / Alert / Toast / Skeleton (already synced)</h2>
      <div class="design-page__row">
        <Specimen label="Badge × variants">
          <div class="design-page__stack">
            <Badge variant="neutral">neutral</Badge>
            <Badge variant="accent">accent</Badge>
            <Badge variant="success">success</Badge>
            <Badge variant="danger">danger</Badge>
          </div>
        </Specimen>
        <Specimen label="Alert × variants" width="300px">
          <div class="design-page__stack">
            <Alert variant="info" title="Info">Heads-up copy.</Alert>
            <Alert variant="danger" title="Unable to sign in"
              >Check your credentials.</Alert
            >
          </div>
        </Specimen>
        <Specimen label="Toast" width="300px">
          <Toast title="Saved" description="Changes are live." />
        </Specimen>
        <Specimen label="Skeleton" width="240px">
          <div class="design-page__stack">
            <Skeleton style="width: 200px" />
            <Skeleton style="width: 160px" />
            <Skeleton style="width: 120px" />
          </div>
        </Specimen>
        <Specimen label="Loader icon">
          <LoaderCircle class="design-page__spin" />
        </Specimen>
      </div>
    </section>
  </DesignLayout>
</template>

<style scoped>
.design-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-page__group {
  margin: 0 0 var(--space-8);
}

.design-page__group h2 {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--space-4);
}

.design-page__stack {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.design-page__spin {
  width: var(--size-spinner);
  height: var(--size-spinner);
  color: var(--ws-text-secondary);
  animation: design-feedback-spin var(--duration-spinner) linear infinite;
}

@keyframes design-feedback-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .design-page__spin {
    animation: none;
  }
}
</style>
