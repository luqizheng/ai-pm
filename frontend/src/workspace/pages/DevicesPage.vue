<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Ban,
  CheckCircle2,
  CircleSlash,
  Link2,
  RefreshCw,
  Unlink
} from "lucide-vue-next";
import PageHeader from "./PageHeader.vue";
import {
  demoDeviceDetails,
  demoDevices
} from "../fixtures";
import {
  getDevice,
  listDevices,
  revokeDevice,
  unbindDevice,
  type Device,
  type DeviceDetail,
  type DeviceReadiness,
  type DeviceStatus
} from "@/api/devices";
import { formatDate } from "@/api/format";

/**
 * 开发环境管理页（T-091 / Codex Bootstrap V1.3 §九）。
 *
 * - 状态全部消费「后端字段」：readiness 六层（installed/connected/authenticated/
 *   ready/degraded/failed）+ 生命周期 status（active/revoked）——**页面零推导**。
 * - V1 无在线徽标、无 Worker 徽标（V2 才有心跳与 Runner）；「最后自检」替代
 *   「最后心跳」；页面显式呈现「就绪是快照」——自检时间与逐项结果可见。
 * - 项目可用性（bindings[].project_accessible）是后端按**当前调用人**派生的
 *   值，前端不计算。
 * - mock 分支与真后端分支在数据层分叉、模板唯一（同构）。
 */

const { t } = useI18n();
const useMock = import.meta.env.VITE_USE_MOCK === "1";

const devices = ref<Device[]>([]);
const loading = ref(false);
const loadError = ref("");
const acting = ref("");

const selectedId = ref("");
const detail = ref<DeviceDetail | null>(null);
const detailLoading = ref(false);

async function refresh() {
  loading.value = true;
  loadError.value = "";
  try {
    if (useMock) {
      devices.value = demoDevices;
    } else {
      const res = await listDevices();
      devices.value = res.items;
    }
    if (!selectedId.value) {
      selectedId.value = devices.value.find(d => d.status === "active")?.id ?? devices.value[0]?.id ?? "";
    }
    await loadDetail();
  } catch (err) {
    loadError.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;
  }
}

async function loadDetail() {
  if (!selectedId.value) {
    detail.value = null;
    return;
  }
  detailLoading.value = true;
  try {
    if (useMock) {
      detail.value = toDetail(devices.value.find(d => d.id === selectedId.value));
    } else {
      detail.value = await getDevice(selectedId.value);
    }
  } catch {
    detail.value = null;
  } finally {
    detailLoading.value = false;
  }
}

/** mock 分支：列表行 + fixtures 详情拼装（保持与真后端响应同构）。 */
function toDetail(row?: Device): DeviceDetail | null {
  if (!row) return null;
  const fixture = demoDeviceDetails[row.id];
  return { ...row, bindings: fixture?.bindings ?? [] };
}

async function revoke(d: Device) {
  if (d.status !== "active") return;
  if (!window.confirm(t("workspace.devices.confirmRevoke", { name: d.name }))) return;
  acting.value = d.id;
  try {
    if (!useMock) await revokeDevice(d.id);
    await refresh();
  } finally {
    acting.value = "";
  }
}

async function unbind(bindingId: string) {
  if (!selectedId.value) return;
  if (!window.confirm(t("workspace.devices.confirmUnbind"))) return;
  acting.value = bindingId;
  try {
    if (!useMock) await unbindDevice(selectedId.value, bindingId);
    await refresh();
  } finally {
    acting.value = "";
  }
}

const activeCount = computed(() => devices.value.filter(d => d.status === "active").length);
const readyCount = computed(
  () => devices.value.filter(d => d.readiness === "ready").length
);

function readinessKey(readiness: DeviceReadiness | null): string {
  return readiness ? `workspace.devices.readiness.${readiness}` : "workspace.devices.readiness.none";
}

function statusKey(status: DeviceStatus): string {
  return `workspace.devices.status.${status}`;
}

function readinessClass(readiness: DeviceReadiness | null): string {
  switch (readiness) {
    case "ready":
      return "is-ready";
    case "degraded":
      return "is-degraded";
    case "failed":
      return "is-failed";
    case null:
      return "is-pending";
    default:
      return "is-partial";
  }
}

onMounted(refresh);
</script>

<template>
  <div class="devices-page">
    <PageHeader :title="t('workspace.devices.title')" :description="t('workspace.devices.subtitle')">
      <template #actions>
        <button class="btn ghost" :disabled="loading" @click="refresh">
          <RefreshCw class="icon" :class="{ spin: loading }" />
          {{ t("common.refresh") }}
        </button>
        <button class="btn primary" @click="scope.jumpToAiCenter?.()">
          <MonitorSmartphone class="icon" />
          {{ t("workspace.devices.add") }}
        </button>
      </template>
    </PageHeader>

    <div class="metrics">
      <div class="metric">
        <span class="metric-value">{{ activeCount }}</span>
        <span class="metric-label">{{ t("workspace.devices.metrics.active") }}</span>
      </div>
      <div class="metric">
        <span class="metric-value">{{ readyCount }}</span>
        <span class="metric-label">{{ t("workspace.devices.metrics.ready") }}</span>
      </div>
      <div class="metric">
        <span class="metric-value">{{ devices.length }}</span>
        <span class="metric-label">{{ t("workspace.devices.metrics.total") }}</span>
      </div>
    </div>

    <p v-if="loadError" class="error">{{ loadError }}</p>

    <div class="layout">
      <ul class="device-list">
        <li
          v-for="d in devices"
          :key="d.id"
          :class="{ selected: d.id === selectedId }"
          @click="selectedId = d.id; loadDetail()"
        >
          <div class="device-head">
            <span class="device-name">{{ d.name }}</span>
            <span class="badge" :class="d.status === 'active' ? 'is-active' : 'is-revoked'">
              <CheckCircle2 v-if="d.status === 'active'" class="icon" />
              <CircleSlash v-else class="icon" />
              {{ t(statusKey(d.status)) }}
            </span>
          </div>
          <div class="device-meta">
            {{ d.executor_kind }}<template v-if="d.executor_version"> {{ d.executor_version }}</template>
            · {{ d.platform }}<template v-if="d.os_version"> · {{ d.os_version }}</template>
          </div>
          <div class="device-status-row">
            <span class="badge readiness" :class="readinessClass(d.readiness)">
              {{ t(readinessKey(d.readiness)) }}
            </span>
            <span class="check-at" v-if="d.last_check_at">
              {{ t("workspace.devices.lastCheck") }} {{ formatDate(d.last_check_at) }}
            </span>
            <span class="check-at muted" v-else>{{ t("workspace.devices.neverChecked") }}</span>
          </div>
        </li>
      </ul>

      <section class="detail" v-if="detail">
        <h3 class="detail-title">{{ detail.name }}</h3>
        <dl class="kv">
          <dt>{{ t("workspace.devices.fields.executor") }}</dt>
          <dd>
            {{ detail.executor_kind }}
            <template v-if="detail.executor_version">· {{ detail.executor_version }}</template>
          </dd>
          <dt>{{ t("workspace.devices.fields.platform") }}</dt>
          <dd>{{ detail.platform }}<template v-if="detail.os_version"> · {{ detail.os_version }}</template></dd>
          <dt>{{ t("workspace.devices.fields.lifecycle") }}</dt>
          <dd>{{ t(statusKey(detail.status)) }}
            <template v-if="detail.revoked_at">（{{ formatDate(detail.revoked_at) }}）</template>
          </dd>
          <dt>{{ t("workspace.devices.fields.readiness") }}</dt>
          <dd>
            <span class="badge readiness" :class="readinessClass(detail.readiness)">
              {{ t(readinessKey(detail.readiness)) }}
            </span>
          </dd>
          <dt>{{ t("workspace.devices.fields.lastCheck") }}</dt>
          <dd>
            <template v-if="detail.last_check_at">
              {{ formatDate(detail.last_check_at) }}
              <span v-if="detail.last_check" class="check-summary">
                · {{ t("workspace.devices.checkSummary", {
                  verified: detail.last_check.verified_repository_count ?? 0,
                  declared: detail.last_check.declared_repository_count ?? 0
                }) }}
              </span>
            </template>
            <template v-else>{{ t("workspace.devices.neverChecked") }}</template>
          </dd>
        </dl>

        <p class="snapshot-note">{{ t("workspace.devices.snapshotNote") }}</p>

        <div class="bindings-head">
          <h4>{{ t("workspace.devices.bindings") }}</h4>
          <button
            v-if="detail.status === 'active'"
            class="btn ghost danger"
            :disabled="acting === detail.id"
            @click="revoke(detail)"
          >
            <Ban class="icon" />
            {{ t("workspace.devices.revoke") }}
          </button>
        </div>

        <p v-if="detail.bindings.length === 0" class="empty">
          {{ t("workspace.devices.noBindings") }}
        </p>
        <ul v-else class="binding-list">
          <li v-for="b in detail.bindings" :key="b.id">
            <div class="binding-row">
              <Link2 class="icon" />
              <code class="project-id">{{ b.project_id.slice(0, 8) }}</code>
              <span class="badge" :class="b.project_accessible ? 'is-ready' : 'is-degraded'">
                {{ t(b.project_accessible ? "workspace.devices.accessible" : "workspace.devices.inaccessible") }}
              </span>
              <span class="badge muted-badge">
                {{ t("workspace.devices.verifiedCount", { verified: b.verified_repositories.length, declared: b.declared_repository_ids.length }) }}
              </span>
              <span class="spacer" />
              <button
                v-if="!b.unbound_at && detail.status === 'active'"
                class="btn ghost danger sm"
                :disabled="acting === b.id"
                @click="unbind(b.id)"
              >
                <Unlink class="icon" />
                {{ t("workspace.devices.unbind") }}
              </button>
            </div>
            <div class="binding-meta">
              {{ t("workspace.devices.boundAt") }} {{ formatDate(b.bound_at) }}
              <template v-if="b.unbound_at"> · {{ t("workspace.devices.unboundAt") }} {{ formatDate(b.unbound_at) }}</template>
            </div>
          </li>
        </ul>
      </section>
      <section class="detail empty-detail" v-else-if="!detailLoading">
        {{ t("workspace.devices.selectOne") }}
      </section>
    </div>
  </div>
</template>

<style scoped>
.devices-page { display: flex; flex-direction: column; gap: 16px; }
.toolbar { display: flex; align-items: center; gap: 12px; }
.toolbar-hint { font-size: 12px; opacity: 0.6; }
.metrics { display: flex; gap: 12px; }
.metric {
  display: flex; flex-direction: column; gap: 2px;
  padding: 10px 16px; border: 1px solid var(--ws-border);
  border-radius: 8px; min-width: 96px;
}
.metric-value { font-size: 20px; font-weight: 600; }
.metric-label { font-size: 12px; opacity: 0.7; }
.layout { display: grid; grid-template-columns: minmax(280px, 380px) 1fr; gap: 16px; align-items: start; }
.device-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
.device-list li {
  border: 1px solid var(--ws-border); border-radius: 8px; padding: 10px 12px;
  cursor: pointer; display: flex; flex-direction: column; gap: 6px;
}
.device-list li.selected { border-color: var(--ws-accent); }
.device-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.device-name { font-weight: 600; }
.device-meta { font-size: 12px; opacity: 0.75; }
.device-status-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.check-at { font-size: 12px; opacity: 0.75; }
.muted { opacity: 0.5; }
.badge {
  display: inline-flex; align-items: center; gap: 4px;
  font-size: 12px; padding: 2px 8px; border-radius: 999px;
  border: 1px solid var(--ws-border);
}
.badge .icon { width: 12px; height: 12px; }
.badge.is-active, .badge.is-ready { color: var(--ws-color-success-text); }
.badge.is-revoked { opacity: 0.6; }
.badge.is-degraded { color: var(--ws-color-warning-text); }
.badge.is-failed { color: var(--ws-color-danger-text); }
.badge.is-pending, .badge.is-partial { opacity: 0.75; }
.muted-badge { opacity: 0.7; }
.detail { border: 1px solid var(--ws-border); border-radius: 8px; padding: 16px; }
.detail-title { margin: 0 0 12px; font-size: 16px; }
.kv { display: grid; grid-template-columns: auto 1fr; gap: 6px 16px; margin: 0 0 12px; }
.kv dt { font-size: 12px; opacity: 0.7; align-self: center; }
.kv dd { margin: 0; font-size: 13px; }
.check-summary { opacity: 0.75; }
.snapshot-note {
  font-size: 12px; opacity: 0.7; margin: 0 0 12px;
  padding: 8px 10px; border-radius: 6px; background: var(--ws-hover-veil);
}
.bindings-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.bindings-head h4 { margin: 0; font-size: 13px; }
.binding-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
.binding-list li { border: 1px dashed var(--ws-border); border-radius: 6px; padding: 8px 10px; }
.binding-row { display: flex; align-items: center; gap: 8px; }
.binding-row .icon { width: 14px; height: 14px; }
.project-id { font-size: 12px; }
.spacer { flex: 1; }
.binding-meta { font-size: 12px; opacity: 0.7; margin-top: 4px; }
.empty { font-size: 13px; opacity: 0.6; }
.empty-detail { opacity: 0.6; font-size: 13px; }
.btn { display: inline-flex; align-items: center; gap: 6px; border-radius: 6px; padding: 6px 12px; font-size: 13px; cursor: pointer; }
.btn.primary { border: 1px solid var(--ws-accent); color: var(--ws-accent); background: transparent; }
.btn.ghost { border: 1px solid var(--ws-border); background: transparent; }
.btn.danger { color: var(--ws-color-danger-text); }
.btn.sm { padding: 3px 8px; font-size: 12px; }
.btn .icon { width: 14px; height: 14px; }
.icon.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.error { color: var(--ws-color-danger-text); font-size: 13px; margin: 0; }
</style>
