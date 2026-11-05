<script setup lang="ts">
import { ChevronRight } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";

/**
 * 区块头部（Command Center 页内共享布局件）。
 *
 * 结构（Pixso「Project Command Center」各卡头 3:7367/3:7477/3:7582 等）：
 *   标题（13px semibold）+ 弱化元信息 chip ｜ 右侧 accent 文字链入口。
 * 文案由调用方翻译后传入；无 actionLabel 时不渲染入口。
 */
interface Props {
  /** 区块标题（如「任务概览」） */
  title: string;
  /** 标题旁的弱化元信息 chip（如「24 个任务」「独立于任务状态」）；不传不渲染 */
  meta?: string;
  /** 右侧文字链入口文案（如「全部任务」）；不传不渲染 */
  actionLabel?: string;
}

withDefaults(defineProps<Props>(), { meta: "", actionLabel: "" });

const emit = defineEmits<{ (e: "action"): void }>();
</script>

<template>
  <div class="pm-section-head">
    <div class="pm-section-head__title-group">
      <h3 class="pm-section-head__title">{{ title }}</h3>
      <Badge v-if="meta" variant="count" size="sm">{{ meta }}</Badge>
    </div>
    <button
      v-if="actionLabel"
      type="button"
      class="pm-section-head__action"
      @click="emit('action')"
    >
      <span>{{ actionLabel }}</span>
      <ChevronRight class="pm-section-head__action-icon" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
.pm-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  min-width: 0;
}

.pm-section-head__title-group {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.pm-section-head__title {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}

.pm-section-head__action {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  flex: none;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-medium);
  color: var(--accent);
  cursor: pointer;
  transition: color var(--duration-fast) var(--ease-standard);
}

.pm-section-head__action:hover {
  color: var(--accent-hover);
}

.pm-section-head__action:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.pm-section-head__action-icon {
  width: var(--size-icon-xxs);
  height: var(--size-icon-xxs);
  flex: none;
}
</style>
