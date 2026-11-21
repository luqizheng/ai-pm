<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Sparkles } from "lucide-vue-next";

import { Badge } from "@/components/ui/badge";
import { useDesignProject } from "@/views/project/design/use-design-project";
import { fetchConsultations, fetchPendingDecisions } from "@/api/collaboration";

/**
 * AI 协作工作区壳（PA-06 · Figma 258:14081，八页签共用）。
 *
 * 组织方式同 S05 ExecutionFrame：子页是**独立路由**，页签条是 RouterLink
 * （保留深链与前进/后退语义）。画布三段式：Main 结构化项目工作 +
 * 页内右栏证据与推理 + 底部统一 AI-PM 输入（CommandInput，所有页签同一条）。
 *
 * 页签「Decisions」带动态计数（画布「2 human decisions」）= consultation
 * pending + approval pending 两类待决合计；无待决时显示纯页签名（空态
 * 不抹历史：已决事项仍在决策中心可查）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const COLLAB_TABS = [
  {
    key: "overview",
    path: "/project/collaboration",
    labelKey: "pa06.tabs.overview"
  },
  {
    key: "execution",
    path: "/project/collaboration/execution",
    labelKey: "pa06.tabs.execution"
  },
  {
    key: "decisions",
    path: "/project/collaboration/decisions",
    labelKey: "pa06.tabs.decisions"
  },
  {
    key: "knowledge",
    path: "/project/collaboration/knowledge",
    labelKey: "pa06.tabs.knowledge"
  },
  {
    key: "plan",
    path: "/project/collaboration/plan",
    labelKey: "pa06.tabs.plan"
  },
  {
    key: "delivery",
    path: "/project/collaboration/delivery",
    labelKey: "pa06.tabs.delivery"
  },
  {
    key: "recovery",
    path: "/project/collaboration/recovery",
    labelKey: "pa06.tabs.recovery"
  },
  {
    key: "authorization",
    path: "/project/collaboration/authorization",
    labelKey: "pa06.tabs.authorization"
  }
] as const;

type TabKey = (typeof COLLAB_TABS)[number]["key"];

const activeKey = computed(() => {
  if (route.path === "/project/collaboration") return "overview";
  const match = COLLAB_TABS.find(
    tab =>
      tab.path !== "/project/collaboration" && route.path.startsWith(tab.path)
  );
  return (match?.key ?? "") as TabKey | "";
});

const pendingCount = ref(0);

async function loadPendingCount() {
  if (!hasProject.value) {
    pendingCount.value = 0;
    return;
  }
  try {
    const [consultations, decisions] = await Promise.all([
      fetchConsultations(projectId.value, "pending"),
      fetchPendingDecisions(projectId.value)
    ]);
    pendingCount.value = consultations.length + decisions.length;
  } catch {
    // 计数失败不阻塞页签渲染（loading 不是执行状态；错误不扩权）
    pendingCount.value = 0;
  }
}

onMounted(loadPendingCount);
watch(projectId, loadPendingCount);

const prompt = ref("");
const sending = ref(false);

/** 底部统一 AI-PM 输入：与壳 CommandBar 同语义——入队跳会话页 */
function sendPrompt() {
  const text = prompt.value.trim();
  if (!text || sending.value) return;
  sending.value = true;
  prompt.value = "";
  void router.push({
    path: "/project/assistant",
    query: { prompt: text }
  });
  sending.value = false;
}
</script>

<template>
  <div class="collab-frame">
    <nav class="collab-tabs" :aria-label="t('pa06.frame.tabsLabel')">
      <RouterLink
        v-for="tab in COLLAB_TABS"
        :key="tab.key"
        :to="tab.path"
        class="collab-tabs__tab"
        :class="{ 'is-active': activeKey === tab.key }"
        :aria-current="activeKey === tab.key ? 'page' : undefined"
        :data-testid="`collab-tab-${tab.key}`"
      >
        <template v-if="tab.key === 'decisions' && pendingCount > 0">
          {{ t("pa06.tabs.decisionsCount", { count: pendingCount }) }}
        </template>
        <template v-else>{{ t(tab.labelKey) }}</template>
      </RouterLink>
    </nav>

    <slot />

    <!-- 底部统一 AI-PM 输入（画布 CommandInput Collapsed：scope tag + 正式变更提示） -->
    <div class="collab-input" data-testid="collab-input">
      <div class="collab-input__context">
        <Sparkles :size="12" aria-hidden="true" />
        <span class="collab-input__label">AI-PM</span>
        <span class="collab-input__scope">{{ $t("pa06.frame.scopeTag") }}</span>
        <span class="collab-input__hint">{{
          $t("pa06.frame.confirmHint")
        }}</span>
      </div>
      <div class="collab-input__row">
        <input
          v-model="prompt"
          type="text"
          class="collab-input__field"
          :placeholder="$t('pa06.frame.promptPlaceholder')"
          :aria-label="$t('pa06.frame.promptPlaceholder')"
          data-testid="collab-input-field"
          @keydown.enter="sendPrompt"
        />
        <button
          type="button"
          class="collab-input__send"
          :disabled="sending || prompt.trim().length === 0"
          :aria-label="$t('pa06.frame.send')"
          data-testid="collab-input-send"
          @click="sendPrompt"
        >
          <Sparkles :size="14" aria-hidden="true" />
        </button>
      </div>
      <Badge variant="neutral" class="collab-input__badge">{{
        $t("pa06.frame.footerNote")
      }}</Badge>
    </div>
  </div>
</template>

<style scoped>
.collab-frame {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.collab-tabs {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  border-bottom: var(--border-1) solid var(--ws-border);
  overflow-x: auto;
}

.collab-tabs__tab {
  padding: 0 var(--space-3);
  height: 2.25rem;
  display: inline-flex;
  align-items: center;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  white-space: nowrap;
  color: var(--text-secondary);
  cursor: pointer;
  text-decoration: none;
}

.collab-tabs__tab.is-active {
  border-bottom-color: var(--ws-accent);
  color: var(--ws-accent);
}

.collab-tabs__tab:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.collab-input {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-3);
}

.collab-input__context {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
}

.collab-input__context svg {
  color: var(--ws-accent);
}

.collab-input__label {
  font-weight: var(--weight-medium);
  color: var(--ws-accent);
}

.collab-input__scope {
  padding: 1px var(--space-1);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  color: var(--text-secondary);
  font-size: 10px;
}

.collab-input__hint {
  color: var(--text-secondary);
}

.collab-input__row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.collab-input__field {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  padding: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.collab-input__field::placeholder {
  color: var(--text-secondary);
}

.collab-input__field:focus {
  outline: none;
}

.collab-input__send {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  border: 0;
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  color: var(--text-primary);
  cursor: pointer;
}

.collab-input__send:disabled {
  color: var(--text-disabled);
  cursor: not-allowed;
}

.collab-input__send:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
}

.collab-input__badge {
  align-self: flex-start;
}
</style>
