<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { LockKeyhole } from "lucide-vue-next";

import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";

/**
 * Team 域受限态（P4-ORG UX 修订 V1.0 / Figma T4「Team access restricted」）。
 *
 * 资格不足（如 Tenant Admin 身份不自动获得 Team 业务访问）时替代页面主体：
 * 「Eligibility · restricted」徽标 + 受限空态 + No implicit access 说明。
 * 这是呈现态而非权限控制——后端授权同矩阵强制。
 */
const { t } = useI18n();
const router = useRouter();

function backToProducts(): void {
  void router.push("/settings/team/products");
}
</script>

<template>
  <div class="team-restricted" data-testid="team-restricted-state">
    <Badge variant="neutral">{{ t("team.restricted.badge") }}</Badge>
    <EmptyState
      :title="t('team.restricted.title')"
      :description="t('team.restricted.description')"
      :icon="LockKeyhole"
      :action-label="t('team.restricted.backToProducts')"
      @action="backToProducts"
    />
    <Alert variant="info" :title="t('team.restricted.noImplicitAccess')">
      {{ t("team.restricted.noImplicitAccessDetail") }}
    </Alert>
  </div>
</template>

<style scoped>
.team-restricted {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-4);
}

.team-restricted > .ui-empty {
  width: min(100%, 40rem);
}
</style>
