<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { attentionItems, demoProject, demoProjectId } from "../fixtures";
import { useWorkspaceStore } from "../store";
import BriefingCard from "./BriefingCard.vue";
import MetricCards from "./MetricCards.vue";
import AttentionList from "./AttentionList.vue";
import GoalStrip from "./GoalStrip.vue";
import DynamicsFeed from "./DynamicsFeed.vue";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const store = useWorkspaceStore();

const projectId = computed(() => String(route.params.projectId ?? ""));
const available = computed(() => projectId.value === demoProjectId);

// setup 阶段写入现场（首渲染即带选中态，与 V1.1 原型一致：默认选中首条待处理）
if (available.value) {
  store.setProject(projectId.value);
  if (!store.selectedAttentionId) {
    store.selectAttention(attentionItems[0].id);
  }
}
</script>

<template>
  <div v-if="available" class="ws-console">
    <header class="ws-console-header">
      <div class="ws-console-title-row">
        <h1 class="ws-console-title">{{ t("workspace.console.title") }}</h1>
        <span class="ws-badge-running">
          {{ t("workspace.console.runningBadge") }}
        </span>
      </div>
      <p class="ws-console-subtitle">
        {{ demoProject.name }} / {{ demoProject.iteration }} /
        {{ t("workspace.console.demoData") }}
      </p>
    </header>
    <BriefingCard />
    <MetricCards />
    <AttentionList />
    <GoalStrip />
    <DynamicsFeed />
  </div>
  <div v-else class="ws-unavailable">
    <h1 class="ws-unavailable-title">
      {{ t("workspace.unavailable.title") }}
    </h1>
    <p class="ws-unavailable-body">{{ t("workspace.unavailable.body") }}</p>
    <button
      type="button"
      class="ws-unavailable-back ws-focusable"
      data-test="unavailable-back"
      @click="router.push('/dashboard')"
    >
      {{ t("workspace.unavailable.back") }}
    </button>
  </div>
</template>

<style scoped>
.ws-console {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 960px;
}

.ws-console-header {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ws-console-title-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.ws-console-title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: 600;
}

/* 运行徽标：成功语义色（绿），非 AI 紫 */
.ws-badge-running {
  font-size: var(--ws-font-size-caption);
  padding: 2px 10px;
  border-radius: 999px;
  border: 1px solid currentcolor;
  color: var(--ws-color-success-text);
}

.ws-console-subtitle {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-unavailable {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 48px 24px;
}

.ws-unavailable-title {
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-unavailable-body {
  margin: 0;
  color: var(--ws-text-secondary);
}

.ws-unavailable-back {
  height: 32px;
  padding: 0 16px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
  color: var(--ws-text-primary);
  cursor: pointer;
  font-size: var(--ws-font-size-body);
}
</style>
