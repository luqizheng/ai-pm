<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import WorkspaceCanvas from "../layout/WorkspaceCanvas.vue";
import { registerWorkspaceComponent } from "../registry/registry";
import { useWorkspaceStore } from "../store";
import FocusSummary from "./FocusSummary.vue";
import FocusTimeline from "./FocusTimeline.vue";
import { returnHref } from "./focusHrefs";
import { buildFocusPreset, resolveFocus } from "./presets";

registerWorkspaceComponent({ id: "FocusSummary", component: FocusSummary });
registerWorkspaceComponent({ id: "FocusTimeline", component: FocusTimeline });

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const store = useWorkspaceStore();

const projectId = computed(() => String(route.params.projectId ?? ""));
const objectType = computed(() => String(route.params.objectType ?? ""));
const objectId = computed(() => String(route.params.id ?? ""));
const resolution = computed(() => resolveFocus(objectType.value));

const spec = computed(() =>
  resolution.value.ok
    ? buildFocusPreset(resolution.value.type, objectId.value, projectId.value)
    : null
);

function goBack() {
  router.push(returnHref(route.query.from, projectId.value));
}

onMounted(() => {
  if (resolution.value.ok) {
    store.setSubject({ type: objectType.value, id: objectId.value });
  }
});
</script>

<template>
  <div class="ws-focus-page">
    <header class="ws-focus-head">
      <button
        type="button"
        class="ws-focus-back ws-focusable"
        data-test="focus-back"
        @click="goBack"
      >
        {{ t("workspace.focus.back") }}
      </button>
      <h2 class="ws-focus-heading">{{ t("workspace.focus.title") }}</h2>
    </header>

    <WorkspaceCanvas v-if="spec" :spec="spec" data-test="focus-canvas" />

    <section v-else class="ws-focus-degraded" data-test="focus-degraded">
      <p class="ws-focus-degraded-text">
        {{ t("workspace.focus.unknownType") }}
      </p>
      <p class="ws-focus-degraded-ref">{{ objectType }} · {{ objectId }}</p>
    </section>
  </div>
</template>

<style scoped>
.ws-focus-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 1200px;
}

.ws-focus-head {
  display: flex;
  align-items: center;
  gap: 12px;
}

.ws-focus-back {
  min-height: 32px;
  padding: 0 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.ws-focus-heading {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-focus-degraded {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 16px;
  border: 1px dashed var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.ws-focus-degraded-text {
  margin: 0;
  color: var(--ws-text-primary);
}

.ws-focus-degraded-ref {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>