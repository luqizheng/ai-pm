<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/TenantIndicator/TopBarSlice + Tooltip" node=33:87752;33:87798 date=2026-11-04 -->
<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Building2 } from "lucide-vue-next";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger
} from "reka-ui";
import {
  TooltipContent,
  TooltipPortal,
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger
} from "reka-ui";

import TenantIdentityCard from "./TenantIdentityCard.vue";

/**
 * 租户上下文指示器（Figma Global/TenantIndicator 33:87752/87798/87799）。
 *
 * 顶栏紧凑芯片（building-2 16 + 组织名 13 Medium，sidebar 底）：
 * hover/focus → Tooltip（「组织 · 组织名 · 身份」，Focus 态 2px accent 描边）；
 * 点击 → 身份卡浮层（TenantIdentityCard 四态）。
 * **只读指示，不是切换器**：不含任何租户切换/管理入口（Shell V1.1 +
 * 设计任务书 §4E 红线）。
 */
interface Props {
  tenant: string;
  /** 当前身份显示串（Tooltip 尾段；缺省不拼） */
  identity?: string;
  /** 身份卡状态透传 */
  email?: string;
  readOnly?: boolean;
  status?: "ready" | "loading" | "unavailable";
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  identity: undefined,
  email: undefined,
  readOnly: false,
  status: "ready",
  disabled: false
});

const { t } = useI18n();

const cardOpen = ref(false);

const tooltipLabel = props.identity
  ? t("global.tenant.tooltipLabel", {
      tenant: props.tenant,
      identity: props.identity
    })
  : props.tenant;
</script>

<template>
  <TooltipProvider :delay-duration="200">
    <TooltipRoot>
      <PopoverRoot v-model:open="cardOpen">
        <TooltipTrigger as-child>
          <PopoverTrigger as-child>
            <button
              type="button"
              class="tenant-indicator"
              :aria-label="tooltipLabel"
              :disabled="props.disabled"
            >
              <Building2 :size="16" aria-hidden="true" />
              <span
                v-if="props.status === 'loading'"
                class="tenant-indicator__loading"
                >{{ t("global.tenant.loading") }}</span
              >
              <span v-else class="tenant-indicator__name">{{
                props.tenant
              }}</span>
            </button>
          </PopoverTrigger>
        </TooltipTrigger>

        <TooltipPortal>
          <TooltipContent
            class="tenant-indicator__tooltip"
            side="bottom"
            align="start"
            :side-offset="4"
          >
            {{ tooltipLabel }}
          </TooltipContent>
        </TooltipPortal>

        <PopoverPortal>
          <PopoverContent
            class="tenant-indicator__card"
            side="bottom"
            align="start"
            :side-offset="4"
          >
            <TenantIdentityCard
              :tenant="props.tenant"
              :email="props.email"
              :read-only="props.readOnly"
              :status="props.status"
            />
          </PopoverContent>
        </PopoverPortal>
      </PopoverRoot>
    </TooltipRoot>
  </TooltipProvider>
</template>

<style scoped>
/* 芯片：h32（壳紧凑控件档）/ sidebar 底 / radius 8 */
.tenant-indicator {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-compact);
  padding: 0 var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  white-space: nowrap;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.tenant-indicator:hover:not(:disabled) {
  background: var(--ws-elevated);
}

.tenant-indicator:focus-visible {
  outline: none;
  border: var(--border-1) solid var(--ws-accent);
  box-shadow: inset 0 0 0 1px var(--ws-accent);
}

.tenant-indicator:disabled {
  cursor: not-allowed;
  opacity: var(--opacity-disabled-control);
}

.tenant-indicator svg {
  flex: none;
  color: var(--ws-text-secondary);
}

.tenant-indicator__name {
  overflow: hidden;
  text-overflow: ellipsis;
}

.tenant-indicator__loading {
  color: var(--ws-text-secondary);
}

:deep(.tenant-indicator__tooltip) {
  display: flex;
  align-items: center;
  height: var(--ws-row-height);
  padding: 0 var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  z-index: 60;
}

.tenant-indicator__card {
  background: transparent;
  border: none;
  padding: 0;
  z-index: 50;
}
</style>
