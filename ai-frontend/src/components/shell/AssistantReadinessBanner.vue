<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { AiReadiness } from "@/api/llm";

/**
 * AI 未就绪引导横幅（R3 / B01-01）。
 *
 * B01-01 的产品面：默认部署 AI 三循环 off + 无 Provider——用户发消息后
 * 永久无回复却毫无提示。本横幅以 `/ai-readiness` 探针（DB 四项数据条件，
 * 不含任何密钥）给出「为什么不可用 + 去哪里配置」，不做任何可用性伪装：
 * - readiness=null（未拉取/探针失败/无项目）→ 不渲染（不制造噪音）；
 * - ready=true → 不渲染；
 * - ready=false → 渲染缺项原因 + 团队设置入口。
 *
 * **诚实边界**：worker 回复/访谈循环开关在部署面，API 如实不观测——
 * ready=true 只表示模型解析的数据条件齐备，横幅不宣称「一定有回复」。
 */
const props = defineProps<{ readiness: AiReadiness | null }>();
const { t } = useI18n();

const visible = computed(
  () => props.readiness !== null && props.readiness.ready === false
);

/** 缺项原因列表（按激活链顺序；行文案 i18n）。 */
const missing = computed(() => {
  const r = props.readiness;
  if (!r || r.ready) return [] as { key: string }[];
  const items: { key: string }[] = [];
  if (r.provider_enabled_count === 0)
    items.push({ key: "assistant.readiness.missingProvider" });
  if (r.catalog_model_enabled_count === 0)
    items.push({ key: "assistant.readiness.missingModel" });
  if (r.team_wide_authorization_count === 0)
    items.push({ key: "assistant.readiness.missingAuthorization" });
  if (!r.general_policy_set)
    items.push({ key: "assistant.readiness.missingGeneralPolicy" });
  return items;
});
</script>

<template>
  <div
    v-if="visible"
    class="readiness"
    role="alert"
    data-testid="assistant-readiness-banner"
  >
    <p class="readiness__title">
      {{ t("assistant.readiness.title") }}
    </p>
    <ul class="readiness__list">
      <li v-for="item in missing" :key="item.key">
        {{ t(item.key) }}
      </li>
    </ul>
    <p class="readiness__hint">{{ t("assistant.readiness.hint") }}</p>
    <RouterLink
      class="readiness__link"
      to="/settings/team/ai-providers"
      data-testid="assistant-readiness-link"
    >
      {{ t("assistant.readiness.settingsLink") }}
    </RouterLink>
  </div>
</template>

<style scoped>
.readiness {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  margin-bottom: var(--space-2);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--ws-color-warning);
  border-radius: var(--radius-md);
}

.readiness__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-small);
  font-weight: var(--weight-semibold);
}

.readiness__list {
  margin: 0;
  padding-left: var(--space-4);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.readiness__hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.readiness__link {
  color: var(--ws-accent);
  font-size: var(--font-size-caption);
}
</style>
