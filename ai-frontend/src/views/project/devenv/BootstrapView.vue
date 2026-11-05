<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { ExternalLink, Plus, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import PageFrame from "@/components/shell/PageFrame.vue";
import { useScopeStore } from "@/api/scope";
import {
  bindDevice,
  getDevice,
  listDevices,
  registerDevice,
  revokeDevice,
  unbindDevice,
  type Device,
  type DeviceDetail,
  type DeviceReadiness
} from "@/api/devices";

/**
 * S01-06 · Codex / MCP Bootstrap（开发环境接入，长期管理页）。
 *
 * 对接真实后端（pm-api pm_devices.rs 六端点 + onboarding 引导页）：
 * - readiness 六层是**服务端计算**快照（checks.rs compute_readiness），页面只读展示，
 *   不采信客户端自报；自检上报属设备侧职责，UI 不伪造检查动作。
 * - 绑定/解绑/登记/吊销是页面的人工操作（V1.2 §4.1 流程的 V1 形态）。
 * - 未接入设备 ≠ 项目不可用（R-S01-01-01：内置 AI-PM 不依赖外部执行设备）。
 * - MCP 连接状态：后端暂无状态源（G-09 挂账），页面不渲染假状态。
 */
const { t } = useI18n();
const route = useRoute();
const scope = useScopeStore();

const devices = ref<Device[]>([]);
const detail = ref<DeviceDetail | null>(null);
const loading = ref(true);
const errorMessage = ref("");
const busy = ref("");
const showRegister = ref(false);

const projectId = computed(() => {
  const q = route.query.project_id;
  if (typeof q === "string" && q) return q;
  return scope.currentProjectId && scope.currentProjectId !== "all"
    ? scope.currentProjectId
    : "";
});

const stats = computed(() => ({
  total: devices.value.filter(d => d.status === "active").length,
  ready: devices.value.filter(d => d.readiness === "ready").length,
  trouble: devices.value.filter(
    d => d.readiness === "degraded" || d.readiness === "failed"
  ).length
}));

const registerForm = ref({
  name: "",
  platform: "windows" as "windows" | "macos" | "linux",
  os_version: "",
  executor_kind: "codex",
  executor_version: ""
});

async function load() {
  loading.value = true;
  errorMessage.value = "";
  try {
    const page = await listDevices();
    devices.value = page.items;
    if (detail.value) {
      detail.value = await getDevice(detail.value.id);
    }
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

async function run(action: string, fn: () => Promise<unknown>) {
  busy.value = action;
  errorMessage.value = "";
  try {
    await fn();
    await load();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error ?? "");
  } finally {
    busy.value = "";
  }
}

function readinessVariant(
  readiness: DeviceReadiness | null
): "success" | "warning" | "danger" | "neutral" {
  if (readiness === "ready") return "success";
  if (readiness === "degraded" || readiness === "failed") return "danger";
  if (readiness) return "warning";
  return "neutral";
}

function submitRegister() {
  void run("register", async () => {
    await registerDevice({
      name: registerForm.value.name,
      platform: registerForm.value.platform,
      os_version: registerForm.value.os_version || null,
      executor_kind: registerForm.value.executor_kind || undefined,
      executor_version: registerForm.value.executor_version || undefined
    });
    showRegister.value = false;
    registerForm.value = {
      name: "",
      platform: "windows",
      os_version: "",
      executor_kind: "codex",
      executor_version: ""
    };
  });
}

async function openDetail(device: Device) {
  busy.value = `detail-${device.id}`;
  try {
    detail.value = await getDevice(device.id);
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error ?? "");
  } finally {
    busy.value = "";
  }
}

function bindToProject(device: Device) {
  void run(`bind-${device.id}`, () =>
    bindDevice(device.id, { project_id: projectId.value })
  );
}

function unbind(deviceId: string, bindingId: string) {
  void run(`unbind-${bindingId}`, () => unbindDevice(deviceId, bindingId));
}

function revoke(device: Device) {
  void run(`revoke-${device.id}`, () => revokeDevice(device.id));
}

onMounted(load);
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s01.devenv.breadcrumb.project') },
      { label: t('s01.devenv.breadcrumb.devenv') }
    ]"
    :title="t('s01.devenv.page.title')"
    :description="t('s01.devenv.page.description')"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw class="w-4 h-4" />
        {{ t("s01.devenv.action.refresh") }}
      </Button>
      <!-- 安装引导：pm-api onboarding 页（/install/{tool}，T-089） -->
      <a
        href="/api/install/codex"
        target="_blank"
        rel="noopener"
        class="inline-flex items-center gap-1 text-sm"
        data-testid="install-guide"
      >
        <ExternalLink class="w-4 h-4" />
        {{ t("s01.devenv.action.installGuide") }}
      </a>
      <Button variant="primary" size="sm" @click="showRegister = !showRegister">
        <Plus class="w-4 h-4" />
        {{ t("s01.devenv.action.register") }}
      </Button>
    </template>

    <Alert v-if="errorMessage" variant="danger" class="mb-4">
      {{ errorMessage }}
    </Alert>

    <!-- 登记表单（首次登记：服务端生成 device id，客户端持久化属安装引导职责） -->
    <section
      v-if="showRegister"
      class="rounded-lg border p-4 mb-4"
      data-testid="register-form"
    >
      <h3 class="text-sm font-semibold mb-3">
        {{ t("s01.devenv.register.title") }}
      </h3>
      <div class="grid gap-3 md:grid-cols-2">
        <label class="grid gap-1 text-sm">
          {{ t("s01.devenv.field.name") }}
          <input
            v-model="registerForm.name"
            data-testid="register-name"
            class="border rounded px-2 py-1"
            type="text"
          />
        </label>
        <label class="grid gap-1 text-sm">
          {{ t("s01.devenv.field.platform") }}
          <select
            v-model="registerForm.platform"
            data-testid="register-platform"
            class="border rounded px-2 py-1"
          >
            <option value="windows">windows</option>
            <option value="macos">macos</option>
            <option value="linux">linux</option>
          </select>
        </label>
        <label class="grid gap-1 text-sm">
          {{ t("s01.devenv.field.osVersion") }}
          <input
            v-model="registerForm.os_version"
            class="border rounded px-2 py-1"
            type="text"
          />
        </label>
        <label class="grid gap-1 text-sm">
          {{ t("s01.devenv.field.executorKind") }}
          <input
            v-model="registerForm.executor_kind"
            class="border rounded px-2 py-1"
            type="text"
          />
        </label>
      </div>
      <div class="flex gap-2 mt-3">
        <Button
          variant="primary"
          size="sm"
          :disabled="busy === 'register' || !registerForm.name"
          data-testid="register-submit"
          @click="submitRegister"
        >
          {{ t("s01.devenv.register.submit") }}
        </Button>
        <Button variant="ghost" size="sm" @click="showRegister = false">
          {{ t("s01.devenv.action.cancel") }}
        </Button>
      </div>
    </section>

    <!-- 概览 -->
    <section class="grid grid-cols-3 gap-3 mb-4">
      <div class="rounded-lg border p-3">
        <div class="text-xs opacity-70">{{ t("s01.devenv.stats.devices") }}</div>
        <div class="text-xl font-semibold">{{ stats.total }}</div>
      </div>
      <div class="rounded-lg border p-3">
        <div class="text-xs opacity-70">{{ t("s01.devenv.stats.ready") }}</div>
        <div class="text-xl font-semibold">{{ stats.ready }}</div>
      </div>
      <div class="rounded-lg border p-3">
        <div class="text-xs opacity-70">{{ t("s01.devenv.stats.trouble") }}</div>
        <div class="text-xl font-semibold">{{ stats.trouble }}</div>
      </div>
    </section>

    <!-- 空态（未接入 ≠ 不可用，R-S01-01-01） -->
    <EmptyState
      v-if="!loading && devices.length === 0"
      :title="t('s01.devenv.empty.title')"
      :description="t('s01.devenv.empty.description')"
      :action-label="t('s01.devenv.action.register')"
      data-testid="devenv-empty"
      @action="showRegister = true"
    />

    <section v-else class="grid gap-3" data-testid="device-list">
      <div
        v-for="device in devices"
        :key="device.id"
        class="rounded-lg border p-3"
      >
        <div class="flex items-center gap-2 flex-wrap">
          <span class="font-medium">{{ device.name }}</span>
          <Badge :variant="readinessVariant(device.readiness)" size="sm">
            {{ t(`s01.devenv.readiness.${device.readiness ?? "unchecked"}`) }}
          </Badge>
          <Badge variant="neutral" size="sm">
            {{ device.platform }} · {{ device.executor_kind }}
          </Badge>
          <span v-if="device.status === 'revoked'" class="text-xs opacity-70">
            {{ t("s01.devenv.badge.revoked") }}
          </span>
        </div>
        <div class="text-xs opacity-70 mt-1">
          {{ t("s01.devenv.badge.lastCheck") }}:
          {{ device.last_check_at ?? t("s01.devenv.badge.never") }}
          <template v-if="device.last_check?.tool">
            · {{ t("s01.devenv.badge.toolReady") }}
          </template>
        </div>
        <div class="flex gap-2 mt-2 flex-wrap">
          <Button
            variant="secondary"
            size="sm"
            :disabled="busy === `detail-${device.id}`"
            :data-testid="`detail-${device.id}`"
            @click="openDetail(device)"
          >
            {{ t("s01.devenv.action.detail") }}
          </Button>
          <Button
            v-if="projectId && device.status === 'active'"
            variant="ghost"
            size="sm"
            :disabled="busy === `bind-${device.id}`"
            :data-testid="`bind-${device.id}`"
            @click="bindToProject(device)"
          >
            {{ t("s01.devenv.action.bindCurrent") }}
          </Button>
          <Button
            v-if="device.status === 'active'"
            variant="ghost"
            size="sm"
            :disabled="busy === `revoke-${device.id}`"
            @click="revoke(device)"
          >
            {{ t("s01.devenv.action.revoke") }}
          </Button>
        </div>

        <!-- 详情：readiness 分层 + 绑定列表 -->
        <div
          v-if="detail && detail.id === device.id"
          class="mt-3 rounded border p-3 text-sm"
          data-testid="device-detail"
        >
          <div class="grid gap-1">
            <div class="flex justify-between">
              <span class="opacity-70">installed</span>
              <span>{{ detail.last_check?.installed ? "✓" : "—" }}</span>
            </div>
            <div class="flex justify-between">
              <span class="opacity-70">connected</span>
              <span>{{ detail.last_check?.connected ? "✓" : "—" }}</span>
            </div>
            <div class="flex justify-between">
              <span class="opacity-70">tool</span>
              <span>{{ detail.last_check?.tool ? "✓" : "—" }}</span>
            </div>
            <div class="flex justify-between">
              <span class="opacity-70">{{ t("s01.devenv.detail.repoCoverage") }}</span>
              <span>
                {{ detail.last_check?.verified_repository_count ?? 0 }} /
                {{ detail.last_check?.declared_repository_count ?? 0 }}
              </span>
            </div>
          </div>
          <h4 class="text-xs font-semibold mt-3 mb-1">
            {{ t("s01.devenv.detail.bindings") }}
          </h4>
          <div
            v-for="binding in detail.bindings.filter(b => !b.unbound_at)"
            :key="binding.id"
            class="flex items-center justify-between gap-2 py-1"
          >
            <span class="text-xs">{{ binding.project_id }}</span>
            <Button
              variant="ghost"
              size="sm"
              :disabled="busy === `unbind-${binding.id}`"
              :data-testid="`unbind-${binding.id}`"
              @click="unbind(device.id, binding.id)"
            >
              {{ t("s01.devenv.action.unbind") }}
            </Button>
          </div>
          <p
            v-if="detail.bindings.filter(b => !b.unbound_at).length === 0"
            class="text-xs opacity-70"
          >
            {{ t("s01.devenv.detail.noBindings") }}
          </p>
        </div>
      </div>
    </section>
  </PageFrame>
</template>
