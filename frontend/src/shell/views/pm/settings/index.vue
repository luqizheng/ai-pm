<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Card } from "@/components/ui/card";
import { PageHeader } from "@/shell/components";

const { t } = useI18n();

const useMock = import.meta.env.VITE_USE_MOCK === "1";
const apiBase = String(import.meta.env.VITE_API_BASE ?? "/api");

const modeText = useMock
  ? t("pm.settings.modeMock")
  : t("pm.settings.modeBackend");
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('pm.settings.pageTitle')"
      :subtitle="t('pm.settings.subtitle')"
    />

    <div class="card-grid">
      <Card class="panel">
        <p class="panel__title">{{ t("pm.settings.mode") }}</p>
        <dl class="defs">
          <div class="defs__row">
            <dt>{{ t("pm.settings.mode") }}</dt>
            <dd>{{ modeText }}</dd>
          </div>
          <div class="defs__row">
            <dt>{{ t("pm.settings.apiBase") }}</dt>
            <dd class="mono">{{ apiBase }}</dd>
          </div>
        </dl>
      </Card>

      <Card class="panel">
        <p class="panel__title">{{ t("pm.settings.backendStatus") }}</p>
        <p class="panel__body">{{ t("pm.settings.backendHint") }}</p>
      </Card>
    </div>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
}

.panel__title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.panel__body {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: 1.7;
  color: var(--text-secondary);
}

.defs {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.defs__row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-4);
}

.defs__row dt {
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.defs__row dd {
  margin: 0;
  font-size: var(--font-size-body);
  color: var(--text-primary);
  text-align: right;
}

.mono {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

@media (max-width: 960px) {
  .card-grid {
    grid-template-columns: 1fr;
  }
}
</style>
