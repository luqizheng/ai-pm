<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Blocks, Scale, UsersRound, Building2 } from "lucide-vue-next";

import {
  TENANT_NAV,
  SettingsPageLayout,
  SettingsPermissionNotice,
  SettingsSidebar,
  useSettingsDemo
} from "@/components/settings";

import Radio from "@/components/ui/radio/Radio.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";

import { DESIGN_PATHS } from "@/router/design";

/**
 * 帧 08 — 租户管理设置落地页（§4B）。
 * 演示角色切换（管理员 / 普通成员）：成员无授权 → 整域
 * permission denied（§6.9）；管理态展示四类目（治理为 Proposed）。
 */
const { t } = useI18n();

const demo = useSettingsDemo();
const denied = computed(() => demo.tenantAccess.value === "hidden");

const icons = {
  tenantGeneral: Building2,
  members: UsersRound,
  integrations: Blocks,
  governance: Scale
};

const personaOptions = [
  { value: "tenant-admin", labelKey: "settings.demo.persona.tenantAdmin" },
  { value: "tenant-member", labelKey: "settings.demo.persona.tenantMember" }
] as const;

const categories = [
  {
    id: "tenantGeneral",
    descKey: "settings.tenant.landing.categoryGeneral",
    to: DESIGN_PATHS.settings.tenantGeneral
  },
  {
    id: "members",
    descKey: "settings.tenant.landing.categoryMembers",
    to: DESIGN_PATHS.settings.tenantMembers
  },
  {
    id: "integrations",
    descKey: "settings.tenant.landing.categoryIntegrations",
    to: DESIGN_PATHS.settings.tenantIntegrations
  },
  {
    id: "governance",
    descKey: "settings.tenant.landing.categoryGovernance",
    to: DESIGN_PATHS.settings.tenantGeneral,
    proposed: true
  }
] as const;

const breadcrumb = [
  { label: t("design.index.title"), to: DESIGN_PATHS.index },
  {
    label: t("settings.architecture.title"),
    to: DESIGN_PATHS.settings.architecture
  },
  { label: t("settings.tenant.landing.title") }
];
</script>

<template>
  <SettingsPageLayout
    :breadcrumb="breadcrumb"
    :title="t('settings.tenant.landing.title')"
    :description="t('settings.tenant.landing.desc')"
    :icons="icons"
  >
    <template #actions>
      <div class="tenant-landing__persona">
        <span class="tenant-landing__persona-label">
          {{ t("settings.demo.personaLabel") }}
        </span>
        <Radio
          v-for="option in personaOptions"
          :key="option.value"
          :model-value="demo.state.tenantPersona"
          :value="option.value"
          :label="t(option.labelKey)"
          name="tenant-persona"
          @update:model-value="
            demo.setTenantPersona($event as 'tenant-admin' | 'tenant-member')
          "
        />
      </div>
    </template>

    <template #sidebar>
      <SettingsPermissionNotice
        v-if="denied"
        access="denied"
        :role="t('settings.demo.persona.tenantMember')"
      />
      <SettingsSidebar
        :scope-label="t('settings.tenant.landing.title')"
        :entries="TENANT_NAV"
        :icons="icons"
        :denied="denied"
      />
    </template>

    <template v-if="denied">
      <div class="tenant-landing__denied">
        <EmptyState
          :title="t('settings.tenant.landing.deniedTitle')"
          :description="t('settings.tenant.landing.deniedDesc')"
        />
        <Button
          variant="secondary"
          size="sm"
          @click="demo.setTenantPersona('tenant-admin')"
        >
          {{ t("settings.demo.persona.tenantAdmin") }}
        </Button>
      </div>
    </template>

    <template v-else>
      <p class="tenant-landing__entry">
        {{ t("settings.architecture.scope.tenant.entry") }}
      </p>
      <section class="tenant-landing__grid">
        <RouterLink
          v-for="category in categories"
          :key="category.id"
          class="tenant-landing__card"
          :to="category.to"
        >
          <component
            :is="icons[category.id]"
            class="tenant-landing__icon"
            aria-hidden="true"
          />
          <h2 class="tenant-landing__card-title">
            {{ t(`settings.nav.${category.id}`) }}
          </h2>
          <p class="tenant-landing__card-desc">
            {{ t(category.descKey) }}
          </p>
          <span
            v-if="'proposed' in category && category.proposed"
            class="tenant-landing__proposed"
            >{{ t("settings.common.proposed") }}</span
          >
        </RouterLink>
      </section>
    </template>
  </SettingsPageLayout>
</template>

<style scoped>
.tenant-landing__persona {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.tenant-landing__persona-label {
  font-size: var(--ws-font-size-micro);
  color: var(--ws-text-secondary);
}

.tenant-landing__denied {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-8) 0;
}

.tenant-landing__entry {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-accent);
}

.tenant-landing__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

.tenant-landing__card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
  text-decoration: none;
  transition: border-color var(--duration-fast) var(--ease-standard);
}

.tenant-landing__card:hover {
  border-color: var(--ws-accent);
}

.tenant-landing__icon {
  width: var(--size-icon-md);
  height: var(--size-icon-md);
  color: var(--ws-accent);
}

.tenant-landing__card-title {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.tenant-landing__card-desc {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.tenant-landing__proposed {
  position: absolute;
  top: var(--space-3);
  right: var(--space-3);
  padding: 0 var(--space-1);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.tenant-landing__card:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

@media (prefers-reduced-motion: reduce) {
  .tenant-landing__card {
    transition: none;
  }
}
</style>
