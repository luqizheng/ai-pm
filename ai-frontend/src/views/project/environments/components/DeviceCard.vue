<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronRight, Laptop, Monitor, Server } from "lucide-vue-next";
import type { Component } from "vue";

import Badge from "@/components/ui/badge/Badge.vue";
import type { Device } from "@/api/devices";
import {
  anomalyKey,
  checkRelativeTime,
  readinessChainSteps,
  readinessLabelKey,
  readinessVariant
} from "../domain";
import ReadinessChain from "./ReadinessChain.vue";

/**
 * DeviceCard —— 执行环境列表的设备卡（线稿结构）：
 * 设备 ID + 就绪徽标 → 名称（平台图标）→ 平台/系统、Runner 元信息 →
 * 就绪阶段链 → 底部（最近自检 / 异常行）。
 *
 * 只渲染 devices.ts 契约里的字段；绑定明细在详情页展示（V1 列表不做 N+1）。
 */
const props = defineProps<{ device: Device }>();

const emit = defineEmits<{ open: [] }>();

const { t } = useI18n();

const PLATFORM_ICONS: Record<Device["platform"], Component> = {
  windows: Monitor,
  macos: Laptop,
  linux: Server
};

const platformIcon = computed(() => PLATFORM_ICONS[props.device.platform]);

const readinessBadge = computed(() => ({
  label: t(readinessLabelKey(props.device)),
  variant: readinessVariant(props.device)
}));

const chain = computed(() => readinessChainSteps(props.device));

const anomaly = computed(() => {
  const key = anomalyKey(props.device);
  return key
    ? {
        key,
        tone:
          props.device.status === "revoked" ||
          props.device.readiness === "failed"
            ? "danger"
            : "warning"
      }
    : null;
});

const lastCheck = computed(() => {
  const relative = checkRelativeTime(props.device.last_check_at);
  return t(relative.key, relative.n ? { n: relative.n } : {});
});

const platformLine = computed(() => {
  const platform = t(`environments.platform.${props.device.platform}`);
  return props.device.os_version
    ? `${platform} · ${props.device.os_version}`
    : platform;
});

const runnerLine = computed(() => {
  const kind = props.device.executor_kind || "—";
  const version = props.device.executor_version
    ? `v${props.device.executor_version.replace(/^v/, "")}`
    : "";
  return `${t("environments.card.runner")}: ${kind} ${version}`.trim();
});
</script>

<template>
  <article
    class="env-card"
    :class="{ 'is-revoked': props.device.status === 'revoked' }"
    data-testid="env-card"
  >
    <button
      type="button"
      class="env-card__open"
      :data-testid="`env-card-open-${props.device.id}`"
      @click="emit('open')"
    >
      <span class="env-card__did">{{ props.device.id }}</span>
      <span class="env-card__head-right">
        <Badge :variant="readinessBadge.variant" size="sm">
          {{ readinessBadge.label }}
        </Badge>
        <ChevronRight class="env-card__chevron" aria-hidden="true" />
      </span>
    </button>

    <div class="env-card__name">
      <span class="env-card__platform" aria-hidden="true">
        <component :is="platformIcon" :size="15" />
      </span>
      <span class="env-card__title">{{ props.device.name }}</span>
    </div>

    <dl class="env-card__meta">
      <div class="env-card__meta-row">
        <dt class="env-card__meta-term">
          {{ t("environments.card.platform") }}
        </dt>
        <dd>{{ platformLine }}</dd>
      </div>
      <div class="env-card__meta-row">
        <dt class="env-card__meta-term">
          {{ t("environments.card.lastCheck") }}
        </dt>
        <dd>{{ lastCheck }}</dd>
      </div>
    </dl>

    <ReadinessChain :steps="chain" compact />

    <p
      v-if="anomaly"
      class="env-card__anomaly"
      :class="`env-card__anomaly--${anomaly.tone}`"
      data-testid="env-card-anomaly"
    >
      {{ t("environments.card.anomalyPrefix") }}
      {{ t(anomaly.key) }}
    </p>
  </article>
</template>

<style scoped>
.env-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--ws-radius-card);
  transition:
    border-color var(--duration-fast) ease,
    box-shadow var(--duration-fast) ease;
}

.env-card:hover {
  border-color: var(--border-strong);
}

.env-card.is-revoked {
  opacity: var(--opacity-disabled-field);
}

.env-card__open {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  width: 100%;
  padding: 0;
  border: none;
  background: none;
  cursor: pointer;
  text-align: left;
}

.env-card__open:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--space-1);
  border-radius: var(--radius-sm);
}

.env-card__did {
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
  font-variant-numeric: tabular-nums;
}

.env-card__head-right {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.env-card__chevron {
  color: var(--ws-text-secondary);
}

.env-card__name {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.env-card__platform {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-compact);
  height: var(--size-control-compact);
  border-radius: var(--radius-sm);
  background-color: var(--ws-color-accent-surface);
  color: var(--accent);
}

.env-card__title {
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.env-card__meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding-left: var(--space-6);
  margin: 0;
}

.env-card__meta-row {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.env-card__meta-term {
  flex: none;
  color: var(--ws-text-disabled);
}

.env-card__meta-row dd {
  margin: 0;
}

.env-card__anomaly {
  margin: 0;
  padding-top: var(--space-2);
  border-top: var(--border-1) dashed var(--border-subtle);
  font-size: var(--font-size-caption);
}

.env-card__anomaly--warning {
  color: var(--ws-color-warning-text);
}

.env-card__anomaly--danger {
  color: var(--ws-color-danger-text);
}

@media (prefers-reduced-motion: reduce) {
  .env-card,
  .env-card:hover {
    transition: none;
  }
}
</style>
