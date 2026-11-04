<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2609 set="02 — Authentication / Password requirements" date=2026-11-04 -->
<script setup lang="ts">
import { Check, Circle } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

/**
 * 口令要求清单 —— 重置密码页的实时校验清单。
 *
 * Figma：每行 = 12px 图标 + 11px 文案，间距 8px。
 * 状态不靠颜色区分：已满足 = 实心对勾，未满足 = 空心圆（对应 Figma
 * 「Never rely on color alone」的无障碍口径）。
 */
export interface AuthRequirement {
  /** 稳定标识 */
  id: string;
  /** 要求文案 */
  label: string;
  /** 是否已满足 */
  met: boolean;
}

interface Props {
  /** 要求条目 */
  items: AuthRequirement[];
}

defineProps<Props>();
</script>

<template>
  <ul class="auth-requirements">
    <li
      v-for="item in items"
      :key="item.id"
      class="auth-requirements__item"
      :data-met="item.met"
    >
      <Check
        v-if="item.met"
        class="auth-requirements__icon"
        aria-hidden="true"
      />
      <Circle v-else class="auth-requirements__icon" aria-hidden="true" />
      <span class="auth-requirements__label">{{ item.label }}</span>
      <span class="ui-sr-only">
        {{ item.met ? t("auth.requirement.met") : t("auth.requirement.unmet") }}
      </span>
    </li>
  </ul>
</template>

<style scoped>
.auth-requirements {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.auth-requirements__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.auth-requirements__icon {
  flex: none;
  width: var(--space-3);
  height: var(--space-3);
  color: var(--ws-text-secondary);
}

.auth-requirements__item[data-met="true"] .auth-requirements__icon {
  color: var(--ws-accent);
}

.auth-requirements__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

/* 读屏专用：状态文本（图标本身对读屏不可见） */
.ui-sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
  border: 0;
}
</style>
