<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import {
  TENANT_NAV,
  SettingsConfirmationDialog,
  SettingsFormSection,
  INTEGRATIONS_FIXTURE,
  useSettingsDemo,
  type DemoIntegration
} from "@/components/settings";
import SettingsFrame from "./SettingsFrame.vue";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 11 — 租户集成（§4B Integrations）。
 * 提供方连接卡片（Git / Webhook / SSO）；SSO 为规划中能力（§9）；
 * 断开连接走破坏性确认（§6.7）；无授权走 denied 态。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const denied = computed(() => demo.tenantAccess.value === "hidden");

const integrations = ref<DemoIntegration[]>(INTEGRATIONS_FIXTURE);
const disconnectTarget = ref<DemoIntegration | null>(null);
const disconnectOpen = computed(() => disconnectTarget.value !== null);

function confirmDisconnect() {
  if (disconnectTarget.value) {
    disconnectTarget.value.connected = false;
  }
  disconnectTarget.value = null;
}

function connect(integration: DemoIntegration) {
  integration.connected = true;
}

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.tenant.landing.title"),
    to: DESIGN_PATHS.settings.tenant
  },
  { label: t("settings.nav.integrations") }
];
</script>

<template>
  <SettingsFrame
    :breadcrumb="breadcrumb"
    :title="t('settings.nav.integrations')"
    :scope-label="t('settings.tenant.landing.title')"
    :entries="TENANT_NAV"
    :denied="denied"
    persona="tenant-member"
  >
    <template v-if="!denied">
      <SettingsFormSection
        :title="t('settings.tenant.integrations.title')"
        :description="t('settings.tenant.integrations.desc')"
      >
        <div class="tenant-integrations__list">
          <Card v-for="integration in integrations" :key="integration.id">
            <div class="tenant-integrations__row">
              <div class="tenant-integrations__info">
                <div class="tenant-integrations__name">
                  <span>{{ integration.name }}</span>
                  <span
                    v-if="integration.proposed"
                    class="tenant-integrations__proposed"
                    >{{ t("settings.common.proposed") }}</span
                  >
                  <Badge
                    :variant="integration.connected ? 'success' : 'neutral'"
                    size="sm"
                  >
                    {{
                      integration.connected
                        ? t("settings.common.connected")
                        : t("settings.common.notConnected")
                    }}
                  </Badge>
                </div>
                <p class="tenant-integrations__desc">
                  {{ t(integration.descKey) }}
                </p>
              </div>
              <div class="tenant-integrations__actions">
                <Button
                  v-if="!integration.connected && !integration.proposed"
                  variant="primary"
                  size="sm"
                  @click="connect(integration)"
                >
                  {{ t("settings.tenant.integrations.connect") }}
                </Button>
                <Button
                  v-if="integration.connected"
                  variant="danger"
                  size="sm"
                  @click="disconnectTarget = integration"
                >
                  {{ t("settings.tenant.integrations.disconnect") }}
                </Button>
                <Button
                  v-if="!integration.connected && integration.proposed"
                  variant="secondary"
                  size="sm"
                  disabled
                >
                  {{ t("settings.tenant.integrations.connect") }}
                </Button>
              </div>
            </div>
          </Card>
        </div>
      </SettingsFormSection>

      <SettingsConfirmationDialog
        :open="disconnectOpen"
        :title="t('settings.tenant.integrations.disconnectTitle')"
        :description="t('settings.tenant.integrations.disconnectDesc')"
        :confirm-label="t('settings.tenant.integrations.disconnect')"
        @update:open="
          value => {
            if (!value) disconnectTarget = null;
          }
        "
        @confirm="confirmDisconnect"
      />
    </template>
  </SettingsFrame>
</template>

<style scoped>
.tenant-integrations__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.tenant-integrations__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.tenant-integrations__info {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.tenant-integrations__name {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.tenant-integrations__proposed {
  padding: 0 var(--space-1);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.tenant-integrations__desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.tenant-integrations__actions {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
}
</style>
