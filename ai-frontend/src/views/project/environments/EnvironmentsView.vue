<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  CircleCheck,
  ExternalLink,
  Monitor,
  Plus,
  RefreshCw,
  TriangleAlert
} from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { SearchInput } from "@/components/ui/search-input";
import { Select } from "@/components/ui/select";
import type { SelectOption } from "@/components/ui/select/types";
import { StatCard } from "@/components/ui/stat-card";
import PageFrame from "@/components/shell/PageFrame.vue";
import { listDevices, type Device } from "@/api/devices";
import DeviceCard from "./components/DeviceCard.vue";
import DeviceWizardDialog from "./components/DeviceWizardDialog.vue";
import { envMetrics, filterDevices } from "./domain";

/**
 * 执行环境（Execution Environments）列表页 —— Runner / 设备执行基础设施管理。
 *
 * 数据面 = devices.ts 契约（pm-api `/v1/pm/devices*`，T-088）：
 * - 指标三件套（总数 / Execution Ready / 异常）由 listDevices 全量派生；
 * - 设备卡展示服务端就绪快照的阶段链（Installed → Connected → Authenticated → 终态），
 *   不渲染后端没有的状态源（心跳 / MCP 连接等挂账项不伪造）；
 * - 登记走添加向导（三步：选平台 → 安装引导 → 注册登记）。
 */
const { t } = useI18n();
const router = useRouter();

const devices = ref<Device[]>([]);
const loading = ref(true);
const errorMessage = ref("");

const query = ref("");
const statusFilter = ref("all");
const platformFilter = ref("all");

const showWizard = ref(false);

const INSTALL_GUIDE_URL = "/api/install/codex";

async function load() {
  loading.value = true;
  errorMessage.value = "";
  try {
    const page = await listDevices();
    devices.value = [...page.items].sort((a, b) => {
      const at = a.last_check_at ? Date.parse(a.last_check_at) : 0;
      const bt = b.last_check_at ? Date.parse(b.last_check_at) : 0;
      if (at !== bt) return bt - at;
      return Date.parse(b.created_at) - Date.parse(a.created_at);
    });
  } catch (error) {
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

const metrics = computed(() => envMetrics(devices.value));

const STATUS_OPTIONS = computed<SelectOption[]>(() => [
  { value: "all", label: t("environments.filter.statusAll") },
  { value: "ready", label: t("environments.readiness.ready") },
  { value: "degraded", label: t("environments.readiness.degraded") },
  { value: "failed", label: t("environments.readiness.failed") },
  { value: "unchecked", label: t("environments.readiness.unchecked") },
  { value: "revoked", label: t("environments.readiness.revoked") }
]);

const PLATFORM_OPTIONS = computed<SelectOption[]>(() => [
  { value: "all", label: t("environments.filter.platformAll") },
  { value: "windows", label: t("environments.platform.windows") },
  { value: "macos", label: t("environments.platform.macos") },
  { value: "linux", label: t("environments.platform.linux") }
]);

const filteredDevices = computed(() =>
  filterDevices(devices.value, {
    query: query.value,
    status: statusFilter.value,
    platform: platformFilter.value
  })
);

const hasAnyDevice = computed(() => devices.value.length > 0);

function clearFilters() {
  query.value = "";
  statusFilter.value = "all";
  platformFilter.value = "all";
}

function openDevice(device: Device) {
  void router.push(`/project/environments/${encodeURIComponent(device.id)}`);
}

function openInstallGuide() {
  window.open(INSTALL_GUIDE_URL, "_blank", "noopener");
}

function onRegistered() {
  void load();
}

onMounted(load);
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('environments.breadcrumb.project') },
      { label: t('environments.breadcrumb.environments') }
    ]"
    :title="t('environments.page.title')"
    :description="t('environments.page.description')"
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
      <Button variant="primary" size="sm" @click="showWizard = true">
        <Plus :size="14" aria-hidden="true" />
        {{ t("environments.action.add") }}
      </Button>
    </template>

    <Alert v-if="errorMessage" variant="danger">
      {{ errorMessage }}
    </Alert>

    <!-- 指标三件套：设备总数 / Execution Ready / 异常设备 -->
    <div class="env-metrics" data-testid="env-metrics">
      <StatCard
        :label="t('environments.metrics.total')"
        :value="String(metrics.total)"
        :icon="Monitor"
        tone="neutral"
        layout="compact"
      />
      <StatCard
        :label="t('environments.metrics.ready')"
        :value="String(metrics.ready)"
        :icon="CircleCheck"
        tone="success"
        layout="compact"
      />
      <StatCard
        :label="t('environments.metrics.trouble')"
        :value="String(metrics.trouble)"
        :icon="TriangleAlert"
        tone="danger"
        layout="compact"
      />
    </div>

    <!-- 筛选工具行 -->
    <div class="env-toolbar">
      <div class="env-toolbar__search">
        <SearchInput
          v-model="query"
          :placeholder="t('environments.filter.searchPlaceholder')"
          :shortcut="''"
          size="sm"
          data-testid="env-search"
        />
      </div>
      <div class="env-toolbar__filter" data-testid="env-filter-status">
        <Select
          v-model="statusFilter"
          :options="STATUS_OPTIONS"
          size="sm"
          :aria-label="t('environments.filter.status')"
        />
      </div>
      <div class="env-toolbar__filter" data-testid="env-filter-platform">
        <Select
          v-model="platformFilter"
          :options="PLATFORM_OPTIONS"
          size="sm"
          :aria-label="t('environments.filter.platform')"
        />
      </div>
    </div>

    <!-- 设备卡片网格 -->
    <div v-if="filteredDevices.length" class="env-grid">
      <DeviceCard
        v-for="device in filteredDevices"
        :key="device.id"
        :device="device"
        @open="openDevice(device)"
      />
    </div>

    <!-- 空态：从未登记 vs 筛选无结果 -->
    <div v-else class="env-empty" data-testid="env-empty">
      <EmptyState
        :title="
          hasAnyDevice
            ? t('environments.empty.filteredTitle')
            : t('environments.empty.title')
        "
        :description="
          hasAnyDevice
            ? t('environments.empty.filteredDescription')
            : t('environments.empty.description')
        "
        :icon="Monitor"
        size="lg"
        :action-label="
          hasAnyDevice
            ? t('environments.empty.clearFilters')
            : t('environments.action.add')
        "
        :secondary-action-label="
          hasAnyDevice ? '' : t('environments.action.installGuide')
        "
        @action="hasAnyDevice ? clearFilters() : (showWizard = true)"
        @secondary-action="openInstallGuide"
      />
    </div>

    <DeviceWizardDialog
      v-model:open="showWizard"
      :install-guide-url="INSTALL_GUIDE_URL"
      @registered="onRegistered"
      @go-detail="openDevice"
    />
  </PageFrame>
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

.env-metrics {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.env-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

.env-toolbar__search {
  width: var(--size-list-search-width);
  min-width: var(--size-search-width);
}

.env-toolbar__filter {
  width: var(--size-notification-filter-width);
}

.env-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(420px, 100%), 1fr));
  gap: var(--space-3);
}

.env-empty {
  display: flex;
  justify-content: center;
  padding: var(--space-8) 0;
}
</style>
