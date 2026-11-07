<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ExternalLink, RefreshCw, TriangleAlert } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { StatusIndicator } from "@/components/ui/status-indicator";
import SettingsAppFrame from "@/views/settings/SettingsAppFrame.vue";
import {
  bindDevice,
  getDevice,
  revokeDevice,
  unbindDevice,
  type DeviceBinding,
  type DeviceDetail
} from "@/api/devices";
import { useScopeStore, SCOPE_ALL } from "@/api/scope";
import {
  bannerKeyPrefix,
  bannerVariant,
  checkRelativeTime,
  healthRows,
  readinessLabelKey,
  readinessVariant,
  type EnvHealthRow,
  type HealthState
} from "../environments/domain";
import { useProjectSettingsFrame } from "./shared";

/**
 * Project Setting · Runner 详情页（原执行环境「页面 B」迁入，
 * /project/environments/:id → /project/settings/runners/:id，2026-11-07）：
 * 就绪横幅（为什么可/不可执行 + 恢复）→ 运行健康检查（自检证据 5 行）→
 * 项目绑定（真实绑定契约）→ 运行信息 → 危险操作（吊销，需显式确认）。
 *
 * 契约口径（devices.ts T-088）：
 * - 健康检查 = 最近自检证据的只读展示（installed/connected/tool/仓库覆盖 +
 *   由 readiness 派生的 Authentication），不伪造 MCP/心跳等无状态源的检查项；
 * - 吊销立即阻止新执行授权；在途 Run 的终止与清理按平台生命周期由服务端处理。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const scope = useScopeStore();
const settingsFrame = useProjectSettingsFrame();

const device = ref<DeviceDetail | null>(null);
const loading = ref(true);
const errorMessage = ref("");
const busy = ref("");
const showRevoke = ref(false);

const INSTALL_GUIDE_URL = "/api/install/codex";

const projectId = computed(() => {
  const query = route.query.project_id;
  if (typeof query === "string" && query) return query;
  return scope.currentProjectId && scope.currentProjectId !== SCOPE_ALL
    ? scope.currentProjectId
    : "";
});

const projectNameById = computed(() => {
  const map = new Map<string, string>();
  for (const project of scope.projects) {
    map.set(project.id, project.name);
  }
  return map;
});

async function load() {
  loading.value = true;
  errorMessage.value = "";
  const id = String(route.params.deviceId ?? "");
  if (!id) {
    errorMessage.value = t("environments.detail.notFound");
    loading.value = false;
    return;
  }
  try {
    const [detail] = await Promise.all([
      getDevice(id),
      scope.ensureLoaded().catch(() => undefined)
    ]);
    device.value = detail;
  } catch (error) {
    device.value = null;
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

const banner = computed(() => {
  if (!device.value) return null;
  return {
    variant: bannerVariant(device.value),
    title: t(`${bannerKeyPrefix(device.value)}.title`),
    description: t(`${bannerKeyPrefix(device.value)}.description`)
  };
});

const readinessBadge = computed(() =>
  device.value
    ? {
        label: t(readinessLabelKey(device.value)),
        variant: readinessVariant(device.value)
      }
    : null
);

const HEALTH_ROW_TONES: Record<
  HealthState,
  "success" | "warning" | "danger" | "neutral"
> = {
  pass: "success",
  warning: "warning",
  fail: "danger",
  unknown: "neutral"
};

const health = computed(() => {
  if (!device.value) return [];
  return healthRows(device.value).map(row => ({
    row,
    label: t(`environments.health.${row.key}`),
    tone: HEALTH_ROW_TONES[row.state],
    note: healthNote(row)
  }));
});

function healthNote(row: EnvHealthRow): string {
  if (row.key === "repository" && row.coverage) {
    return t("environments.health.repoCoverage", {
      verified: row.coverage.verified,
      declared: row.coverage.declared
    });
  }
  return t(`environments.health.note.${row.state}`);
}

const runtimeRows = computed(() => {
  const detail = device.value;
  if (!detail) return [];
  const lastCheck = checkRelativeTime(detail.last_check_at);
  const rows: Array<{ key: string; value: string }> = [
    {
      key: "platform",
      value: t(`environments.platform.${detail.platform}`)
    },
    { key: "osVersion", value: detail.os_version || "—" },
    { key: "executorKind", value: detail.executor_kind || "—" },
    {
      key: "executorVersion",
      value: detail.executor_version
        ? `v${detail.executor_version.replace(/^v/, "")}`
        : "—"
    },
    { key: "deviceId", value: detail.id },
    {
      key: "createdAt",
      value: formatDate(detail.created_at)
    },
    {
      key: "lastCheckAt",
      value: detail.last_check_at
        ? `${t(lastCheck.key, lastCheck.n ? { n: lastCheck.n } : {})} · ${formatDate(detail.last_check_at)}`
        : t("environments.time.never")
    }
  ];
  return rows;
});

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  });
}

function bindingProjectName(binding: DeviceBinding): string {
  return projectNameById.value.get(binding.project_id) ?? binding.project_id;
}

function bindingCoverage(binding: DeviceBinding): string {
  const declared = binding.declared_repository_ids.length;
  const verified = binding.verified_repositories.length;
  return t("environments.bindings.coverage", { verified, declared });
}

const canBindCurrent = computed(
  () =>
    Boolean(device.value) &&
    device.value?.status === "active" &&
    Boolean(projectId.value) &&
    !device.value?.bindings.some(
      binding => binding.project_id === projectId.value
    )
);

async function run(action: string, fn: () => Promise<unknown>) {
  busy.value = action;
  errorMessage.value = "";
  try {
    await fn();
    await load();
  } catch (error) {
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    busy.value = "";
  }
}

function bindCurrent() {
  if (!device.value || !projectId.value) return;
  void run("bind", () =>
    bindDevice(device.value!.id, { project_id: projectId.value })
  );
}

function unbind(binding: DeviceBinding) {
  if (!device.value) return;
  void run(`unbind-${binding.id}`, () =>
    unbindDevice(device.value!.id, binding.id)
  );
}

function confirmRevoke() {
  if (!device.value) return;
  showRevoke.value = false;
  void run("revoke", () => revokeDevice(device.value!.id));
}

function backToList() {
  void router.push("/project/settings/runners");
}

onMounted(load);
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="[
      ...settingsFrame.baseCrumbs(),
      { label: t('settings.nav.runners'), to: '/project/settings/runners' },
      { label: device?.name ?? String(route.params.deviceId ?? '') }
    ]"
    :title="device?.name ?? t('environments.detail.title')"
    :scope-label="settingsFrame.scopeLabel.value"
    :entries="settingsFrame.navEntries"
    :icons="settingsFrame.navIcons"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("environments.action.refresh") }}
      </Button>
      <a
        :href="INSTALL_GUIDE_URL"
        target="_blank"
        rel="noopener"
        class="env-link"
        data-testid="env-install-guide"
      >
        <ExternalLink :size="14" aria-hidden="true" />
        {{ t("environments.action.installGuide") }}
      </a>
      <Button
        v-if="!device || device.status === 'active'"
        variant="secondary"
        size="sm"
        :disabled="!canBindCurrent || busy === 'bind'"
        :title="
          canBindCurrent
            ? undefined
            : t('environments.bindings.bindDisabledHint')
        "
        data-testid="env-bind-current"
        @click="bindCurrent"
      >
        {{ t("environments.bindings.bindCurrent") }}
      </Button>
    </template>

    <Alert v-if="errorMessage && !device" variant="danger">
      {{ errorMessage }}
    </Alert>

    <template v-if="device">
      <!-- 就绪横幅：为什么可/不可执行 + 恢复动作 -->
      <Alert
        v-if="banner"
        :variant="banner.variant"
        :title="banner.title"
        data-testid="env-banner"
      >
        {{ banner.description }}
      </Alert>

      <div class="env-detail__head" data-testid="env-detail-head">
        <Badge :variant="readinessBadge?.variant ?? 'neutral'" size="md">
          {{ readinessBadge?.label }}
        </Badge>
        <span class="env-detail__did">{{ device.id }}</span>
        <Badge v-if="device.status === 'revoked'" variant="danger" size="md">
          {{ t("environments.readiness.revoked") }}
        </Badge>
      </div>

      <div class="env-detail__grid">
        <!-- 左列：健康检查 + 项目绑定 -->
        <div class="env-detail__col">
          <section class="env-panel" data-testid="env-health">
            <h2 class="env-panel__title">
              {{ t("environments.health.title") }}
            </h2>
            <ul class="env-health">
              <li
                v-for="item in health"
                :key="item.row.key"
                class="env-health__row"
              >
                <StatusIndicator
                  :status="item.tone"
                  size="md"
                  :label="item.label"
                />
                <span class="env-health__note">{{ item.note }}</span>
              </li>
            </ul>
            <p class="env-panel__footnote">
              {{ t("environments.health.evidenceNote") }}
            </p>
          </section>

          <section class="env-panel" data-testid="env-bindings">
            <div class="env-panel__head">
              <h2 class="env-panel__title">
                {{ t("environments.bindings.title") }}
              </h2>
              <span class="env-panel__hint">{{
                t("environments.bindings.permissionNote")
              }}</span>
            </div>
            <p
              v-if="!device.bindings.length"
              class="env-panel__empty"
              data-testid="env-bindings-empty"
            >
              {{ t("environments.bindings.empty") }}
            </p>
            <ul v-else class="env-bindings">
              <li
                v-for="binding in device.bindings"
                :key="binding.id"
                class="env-bindings__row"
              >
                <div class="env-bindings__main">
                  <span class="env-bindings__project">{{
                    bindingProjectName(binding)
                  }}</span>
                  <span class="env-bindings__meta">
                    {{ bindingCoverage(binding) }} ·
                    {{ t("environments.bindings.boundAt") }}
                    {{ formatDate(binding.bound_at) }}
                  </span>
                </div>
                <Badge
                  v-if="binding.project_accessible !== undefined"
                  :variant="binding.project_accessible ? 'success' : 'neutral'"
                  size="sm"
                >
                  {{
                    binding.project_accessible
                      ? t("environments.bindings.accessible")
                      : t("environments.bindings.notAccessible")
                  }}
                </Badge>
                <Button
                  v-if="device.status === 'active'"
                  variant="ghost"
                  size="sm"
                  :disabled="busy === `unbind-${binding.id}`"
                  :data-testid="`env-unbind-${binding.id}`"
                  @click="unbind(binding)"
                >
                  {{ t("environments.bindings.unbind") }}
                </Button>
              </li>
            </ul>
          </section>
        </div>

        <!-- 右列：运行信息 + 危险操作 -->
        <div class="env-detail__col">
          <section class="env-panel" data-testid="env-runtime">
            <h2 class="env-panel__title">
              {{ t("environments.runtime.title") }}
            </h2>
            <dl class="env-runtime">
              <div
                v-for="row in runtimeRows"
                :key="row.key"
                class="env-runtime__row"
              >
                <dt>{{ t(`environments.runtime.${row.key}`) }}</dt>
                <dd>{{ row.value }}</dd>
              </div>
            </dl>
          </section>

          <section
            v-if="device.status === 'active'"
            class="env-panel env-panel--danger"
            data-testid="env-danger"
          >
            <h2 class="env-panel__title env-panel__title--danger">
              <TriangleAlert aria-hidden="true" :size="15" />
              {{ t("environments.danger.title") }}
            </h2>
            <p class="env-panel__footnote">
              {{ t("environments.danger.description") }}
            </p>
            <Button
              variant="danger"
              size="sm"
              data-testid="env-revoke-open"
              @click="showRevoke = true"
            >
              {{ t("environments.danger.revoke") }}
            </Button>
          </section>
          <section
            v-else
            class="env-panel env-panel--danger"
            data-testid="env-danger"
          >
            <h2 class="env-panel__title env-panel__title--danger">
              <TriangleAlert aria-hidden="true" :size="15" />
              {{ t("environments.danger.revokedTitle") }}
            </h2>
            <p class="env-panel__footnote">
              {{ t("environments.danger.revokedAt") }}
              {{ formatDate(device.revoked_at) }}
            </p>
          </section>
        </div>
      </div>

      <!-- 吊销确认（破坏性操作：显式确认 + 审计） -->
      <ConfirmationDialog
        :open="showRevoke"
        variant="danger"
        :title="t('environments.revoke.title', { id: device.id })"
        :description="t('environments.revoke.description')"
        :confirm-label="t('environments.revoke.confirm')"
        :cancel-label="t('environments.revoke.cancel')"
        :checkbox-label="t('environments.revoke.checkbox')"
        :disabled="busy === 'revoke'"
        data-testid="env-revoke-dialog"
        @update:open="showRevoke = $event"
        @confirm="confirmRevoke"
      />
    </template>

    <template v-if="errorMessage && device">
      <Alert variant="danger">{{ errorMessage }}</Alert>
    </template>
  </SettingsAppFrame>
</template>

<style scoped>
.env-link {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-body);
  color: var(--accent);
  text-decoration: none;
}

.env-link:hover {
  text-decoration: underline;
}

.env-link:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--space-1);
  border-radius: var(--radius-xs);
}

.env-detail__head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.env-detail__did {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.env-detail__grid {
  display: grid;
  grid-template-columns: minmax(0, 7fr) minmax(0, 5fr);
  gap: var(--space-4);
  align-items: start;
}

@media (max-width: 1100px) {
  .env-detail__grid {
    grid-template-columns: minmax(0, 1fr);
  }
}

.env-detail__col {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.env-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--ws-radius-card);
}

.env-panel--danger {
  border-color: var(--ws-color-danger-border);
}

.env-panel__head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-3);
}

.env-panel__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.env-panel__title--danger {
  color: var(--ws-color-danger-text);
}

.env-panel__hint {
  font-size: var(--font-size-micro);
  color: var(--ws-text-disabled);
}

.env-panel__footnote {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.env-panel__empty {
  margin: 0;
  padding: var(--space-4) 0;
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  text-align: center;
}

.env-health {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.env-health__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  min-height: var(--size-row);
  padding: var(--space-2) 0;
  border-top: var(--border-1) solid var(--border-subtle);
}

.env-health__row:first-child {
  border-top: none;
}

.env-health__note {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  text-align: right;
}

.env-bindings {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.env-bindings__row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-top: var(--border-1) solid var(--border-subtle);
}

.env-bindings__row:first-child {
  border-top: none;
}

.env-bindings__main {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.env-bindings__project {
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.env-bindings__meta {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.env-runtime {
  display: flex;
  flex-direction: column;
  margin: 0;
}

.env-runtime__row {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  min-height: var(--size-row);
  padding: var(--space-2) 0;
  border-top: var(--border-1) solid var(--border-subtle);
  font-size: var(--font-size-caption);
}

.env-runtime__row:first-child {
  border-top: none;
}

.env-runtime__row dt {
  flex: none;
  width: var(--size-form-rail-width);
  color: var(--ws-text-secondary);
}

.env-runtime__row dd {
  margin: 0;
  color: var(--ws-text-primary);
  word-break: break-all;
}
</style>
