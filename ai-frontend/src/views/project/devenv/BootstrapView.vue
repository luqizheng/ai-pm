<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { ExternalLink, Plus, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { StatCard } from "@/components/ui/stat-card";
import PageFrame from "@/components/shell/PageFrame.vue";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
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
  return scope.currentProjectId && scope.currentProjectId !== SCOPE_ALL
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
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
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
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
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
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
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

// 项目上下文变化（壳切项目 / 路由带入 project_id）必须重载——
// 该页读模型是项目级的，不重载会停留在上一个项目的数据。
watch(projectId, load);
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
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("s01.devenv.action.refresh") }}
      </Button>
      <!-- 安装引导：pm-api onboarding 页（/install/{tool}，T-089） -->
      <a
        href="/api/install/codex"
        target="_blank"
        rel="noopener"
        class="dv-guide"
        data-testid="install-guide"
      >
        <ExternalLink :size="14" aria-hidden="true" />
        {{ t("s01.devenv.action.installGuide") }}
      </a>
      <Button variant="primary" size="sm" @click="showRegister = !showRegister">
        <Plus :size="14" aria-hidden="true" />
        {{ t("s01.devenv.action.register") }}
      </Button>
    </template>

    <Alert v-if="errorMessage" variant="danger">
      {{ errorMessage }}
    </Alert>

    <!-- 登记表单（首次登记：服务端生成 device id，客户端持久化属安装引导职责） -->
    <section v-if="showRegister" class="dv-panel" data-testid="register-form">
      <h3 class="dv-panel__title">
        {{ t("s01.devenv.register.title") }}
      </h3>
      <div class="dv-panel__grid">
        <label class="dv-field">
          {{ t("s01.devenv.field.name") }}
          <input
            v-model="registerForm.name"
            data-testid="register-name"
            class="dv-input"
            type="text"
          />
        </label>
        <label class="dv-field">
          {{ t("s01.devenv.field.platform") }}
          <select
            v-model="registerForm.platform"
            data-testid="register-platform"
            class="dv-input"
          >
            <option value="windows">windows</option>
            <option value="macos">macos</option>
            <option value="linux">linux</option>
          </select>
        </label>
        <label class="dv-field">
          {{ t("s01.devenv.field.osVersion") }}
          <input
            v-model="registerForm.os_version"
            class="dv-input"
            type="text"
          />
        </label>
        <label class="dv-field">
          {{ t("s01.devenv.field.executorKind") }}
          <input
            v-model="registerForm.executor_kind"
            class="dv-input"
            type="text"
          />
        </label>
      </div>
      <div class="dv-actions">
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

    <!-- 概览（管理摘要：活跃 / 就绪 / 异常） -->
    <section class="dv-stats">
      <StatCard
        :label="t('s01.devenv.stats.devices')"
        :value="String(stats.total)"
        tone="neutral"
      />
      <StatCard
        :label="t('s01.devenv.stats.ready')"
        :value="String(stats.ready)"
        tone="success"
      />
      <StatCard
        :label="t('s01.devenv.stats.trouble')"
        :value="String(stats.trouble)"
        :tone="stats.trouble > 0 ? 'danger' : 'neutral'"
      />
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

    <section v-else class="dv-list" data-testid="device-list">
      <article v-for="device in devices" :key="device.id" class="dv-card">
        <header class="dv-card__head">
          <span class="dv-card__name">{{ device.name }}</span>
          <Badge :variant="readinessVariant(device.readiness)" size="sm">
            {{ t(`s01.devenv.readiness.${device.readiness ?? "unchecked"}`) }}
          </Badge>
          <Badge variant="neutral" size="sm">
            {{ device.platform }} · {{ device.executor_kind }}
          </Badge>
          <span v-if="device.status === 'revoked'" class="dv-card__revoked">
            {{ t("s01.devenv.badge.revoked") }}
          </span>
        </header>
        <p class="dv-card__meta">
          {{ t("s01.devenv.badge.lastCheck") }}:
          {{ device.last_check_at ?? t("s01.devenv.badge.never") }}
          <template v-if="device.last_check?.tool">
            · {{ t("s01.devenv.badge.toolReady") }}
          </template>
        </p>
        <div class="dv-actions">
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
          class="dv-detail"
          data-testid="device-detail"
        >
          <dl class="dv-kv">
            <div class="dv-kv__row">
              <dt>installed</dt>
              <dd>{{ detail.last_check?.installed ? "✓" : "—" }}</dd>
            </div>
            <div class="dv-kv__row">
              <dt>connected</dt>
              <dd>{{ detail.last_check?.connected ? "✓" : "—" }}</dd>
            </div>
            <div class="dv-kv__row">
              <dt>tool</dt>
              <dd>{{ detail.last_check?.tool ? "✓" : "—" }}</dd>
            </div>
            <div class="dv-kv__row">
              <dt>{{ t("s01.devenv.detail.repoCoverage") }}</dt>
              <dd class="dv-mono">
                {{ detail.last_check?.verified_repository_count ?? 0 }} /
                {{ detail.last_check?.declared_repository_count ?? 0 }}
              </dd>
            </div>
          </dl>
          <h4 class="dv-detail__subtitle">
            {{ t("s01.devenv.detail.bindings") }}
          </h4>
          <div
            v-for="binding in detail.bindings.filter(b => !b.unbound_at)"
            :key="binding.id"
            class="dv-binding"
          >
            <span class="dv-mono">{{ binding.project_id }}</span>
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
            class="dv-card__meta"
          >
            {{ t("s01.devenv.detail.noBindings") }}
          </p>
        </div>
      </article>
    </section>
  </PageFrame>
</template>

<style scoped>
.dv-guide {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
  padding: 0 var(--space-3);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-decoration: none;
}

.dv-guide:hover {
  background: var(--surface-sunken);
  color: var(--ws-text-primary);
}

.dv-guide:focus-visible {
  outline: var(--size-focus-ring, 2px) solid var(--ws-accent);
  outline-offset: 1px;
}

.dv-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.dv-panel__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.dv-panel__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

@media (max-width: 880px) {
  .dv-panel__grid {
    grid-template-columns: minmax(0, 1fr);
  }
}

.dv-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.dv-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
}

@media (max-width: 880px) {
  .dv-stats {
    grid-template-columns: minmax(0, 1fr);
  }
}

.dv-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.dv-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.dv-card__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.dv-card__name {
  color: var(--ws-text-primary);
  font-weight: var(--weight-semibold);
}

.dv-card__revoked {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.dv-card__meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.dv-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.dv-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-sunken);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.dv-detail__subtitle {
  margin: var(--space-2) 0 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.dv-kv {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
}

.dv-kv__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.dv-kv__row dt {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.dv-kv__row dd {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.dv-binding {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-1) 0;
}

.dv-mono {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

/* 页面级表单控件（登记表单；跟随 token，不引全局样式） */
.dv-input {
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  background: var(--ws-canvas);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.dv-input:focus-visible {
  outline: var(--size-focus-ring, 2px) solid var(--ws-accent);
  outline-offset: 0;
}
</style>
