<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/TenantIndicator/Identity" node=33:87799 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { Building2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import Badge from "@/components/ui/badge/Badge.vue";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";

/**
 * 租户身份卡（Figma Global/TenantIndicator/Identity 33:87799）。
 *
 * 四态：Full（身份行 + 组织串 + 邮箱 + 可选只读徽标）/ Compact（仅组织名）/
 * Loading（13px 提示 + 骨架行）/ Unavailable（标题 + 说明）。
 * 组织行 = sidebar 底芯片（building-2 16 + 13 Medium）；
 * 身份只读展示，不含任何切换/管理入口（Shell V1.1 口径）。
 */
interface Props {
  tenant: string;
  /** 当前身份邮箱（Full 态；缺省回退 Compact 态） */
  email?: string;
  /** 只读身份徽标（服务账号/系统身份等） */
  readOnly?: boolean;
  status?: "ready" | "loading" | "unavailable";
}

const props = withDefaults(defineProps<Props>(), {
  email: undefined,
  readOnly: false,
  status: "ready"
});

const { t } = useI18n();

const tenantLine = computed(() =>
  t("global.usermenu.tenantLabel", { tenant: props.tenant })
);

const identityLine = computed(() =>
  t("global.tenant.emailLabel", { email: props.email ?? "" })
);
</script>

<template>
  <section class="tenant-card" :aria-label="t('global.tenant.label')">
    <!-- Loading -->
    <template v-if="props.status === 'loading'">
      <p class="tenant-card__hint">{{ t("global.tenant.loading") }}</p>
      <div
        v-for="n in 2"
        :key="n"
        class="tenant-card__skeleton-row"
        aria-hidden="true"
      >
        <Skeleton class="tenant-card__skeleton-line" tone="strong" />
        <Skeleton class="tenant-card__skeleton-line" tone="base" />
      </div>
    </template>

    <!-- Unavailable -->
    <template v-else-if="props.status === 'unavailable'">
      <div class="tenant-card__row">
        <Building2 :size="16" aria-hidden="true" />
        <div class="tenant-card__labels">
          <p class="tenant-card__hint-strong">{{ t("global.tenant.loading") }}</p>
          <p class="tenant-card__hint">{{ t("global.tenant.unavailableDesc") }}</p>
        </div>
      </div>
    </template>

    <!-- Full（有身份邮箱）/ Compact（仅组织名） -->
    <template v-else>
      <div class="tenant-card__row">
        <Building2 :size="16" aria-hidden="true" />
        <span class="tenant-card__tenant">{{ props.tenant }}</span>
      </div>
      <p class="tenant-card__meta">{{ tenantLine }}</p>
      <template v-if="props.email">
        <p class="tenant-card__meta">{{ identityLine }}</p>
        <Badge v-if="props.readOnly" variant="neutral" size="sm">
          {{ t("global.tenant.readOnlyBadge") }}
        </Badge>
      </template>
    </template>
  </section>
</template>

<style scoped>
.tenant-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-3);
  width: var(--size-tenant-card-width);
  max-width: calc(100vw - var(--space-8));
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

/* 组织身份行（sidebar 底芯片） */
.tenant-card__row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  color: var(--ws-text-primary);
  background: var(--ws-sidebar);
  border-radius: var(--radius-sm);
}

.tenant-card__tenant {
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
}

.tenant-card__meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.tenant-card__hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.tenant-card__hint-strong {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.tenant-card__labels {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.tenant-card__skeleton-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  height: var(--ws-row-height);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.tenant-card__skeleton-line {
  height: var(--space-2);
}

.tenant-card__skeleton-line:first-child {
  width: 45%;
}

.tenant-card__skeleton-line:last-child {
  width: 30%;
}
</style>
