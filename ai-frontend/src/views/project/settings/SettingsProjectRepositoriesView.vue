<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  BadgeCheck,
  ChevronRight,
  CircleCheck,
  GitBranch,
  Plus,
  RefreshCw,
  TriangleAlert,
  X
} from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { StatCard } from "@/components/ui/stat-card";
import SettingsAppFrame from "@/views/settings/SettingsAppFrame.vue";
import {
  confirmRepositoryVersion,
  createRepositoryV2,
  disconnectRepository,
  listRepositoriesV2,
  updateRepositoryV2,
  verifyRepositoryV2,
  type RepositoryV2
} from "@/api/initialization";
import { useProjectSettingsFrame } from "./shared";

/**
 * Project Setting · Git Repositories 节（V1.0 稿）——原 S01-03 Repository
 * Management 长期管理页迁入（/project/repository/overview →
 * /project/settings/repositories，2026-11-07 统一入口裁决）。
 *
 * 对接真实后端（pm-api pm_repositories.rs）：
 * - 四概念治理（V1.2 §5.1）：default_branch / selected_commit（Confirmed，仅
 *   人工经 update-version 写入）/ head_commit_sha（Observed，verify 回写）；
 *   `head_commit_sha !== selected_commit` = New commits detected → 只提示，
 *   人工「确认更新」（R-03-04）。
 * - Primary 无后端字段（G-03 挂账）：由 `role === "main"` 唯一性**派生**建议标记，
 *   0 或 ≥2 个 main 都显示「未指定主仓库（可选）」，不判失败（R-03-02）。
 * - 软解绑 = archive（R-03-01 词表 main/docs/service/infra；恢复入口待裁决不提供），
 *   两段式确认（V1.2 §4.4 / §11 Remove 确认）。
 * - OAuth 授权不可用（G-04 挂账）：接入走 URL + 项目 SSH Key（§3.2.2 裁决）。
 * - 列表/筛选/详情布局 = V1.2 §6.1/§12 线框；Discovery 维度后端未落地（§9 分维，
 *   挂账），列表以 Last Verified 代替 Discovery 列，不渲染假状态。
 */
const { t } = useI18n();
const settingsFrame = useProjectSettingsFrame();

const repositories = ref<RepositoryV2[]>([]);
const loading = ref(true);
const errorMessage = ref("");
const busy = ref("");
const selectedId = ref("");
const showConnect = ref(false);

/* ── 筛选（V1.2 §6.4：名称/URL 搜索 + Role + Access；Provider 列后端无字段不设） ── */
const search = ref("");
const roleFilter = ref("");
const statusFilter = ref("");

/* 项目上下文 = 设置域单一来源（?project_id= 优先，回退 scope；SCOPE_ALL 视为无）。
   迁入前此处把哨兵误比 "all"（实际 "__all__"），无项目时会以 __all__ 请求 → 400。 */
const projectId = settingsFrame.projectId;

/* ── 派生：Primary（G-03 前端派生）与基准前移提示（G-05） ── */

/** 仅当恰好一个 `role === "main"` 时给建议标记；0/≥2 → null（V1.2 §2.2）。 */
const primaryRepositoryId = computed(() => {
  const mains = repositories.value.filter(r => r.role === "main");
  return mains.length === 1 ? mains[0]!.id : null;
});

/** New commits detected：Observed HEAD 与 Confirmed 基准不一致（或基准未确认）。 */
function hasNewHead(row: RepositoryV2): boolean {
  if (row.status === "archived" || !row.head_commit_sha) return false;
  return row.selected_commit !== row.head_commit_sha;
}

const stats = computed(() => ({
  linked: repositories.value.filter(r => r.status !== "archived").length,
  verified: repositories.value.filter(r => r.status === "verified").length,
  attention: repositories.value.filter(
    r => r.status === "unreachable" || r.status === "pending"
  ).length,
  confirmed: repositories.value.filter(
    r => r.status !== "archived" && r.selected_commit
  ).length
}));

const filteredRepositories = computed(() => {
  const needle = search.value.trim().toLowerCase();
  return repositories.value.filter(row => {
    if (
      needle &&
      !row.name.toLowerCase().includes(needle) &&
      !row.url.toLowerCase().includes(needle)
    ) {
      return false;
    }
    if (roleFilter.value && row.role !== roleFilter.value) return false;
    if (statusFilter.value && row.status !== statusFilter.value) return false;
    return true;
  });
});

const selected = computed(
  () => repositories.value.find(r => r.id === selectedId.value) ?? null
);

const roleOptions: Array<"main" | "docs" | "service" | "infra"> = [
  "main",
  "docs",
  "service",
  "infra"
];

const statusOptions: Array<RepositoryV2["status"]> = [
  "pending",
  "verified",
  "unreachable",
  "archived"
];

/* ── 接入表单（V1.2 §4.1 V1 形态：URL + 项目 SSH Key；OAuth 不可用不渲染） ── */
const form = ref({
  name: "",
  url: "",
  role: "main" as (typeof roleOptions)[number],
  default_branch: "",
  selected_branch: "",
  selected_commit: "",
  description: ""
});

/* ── 软解绑两段式确认（V1.2 §4.4：第一击武装、第二击执行；换选中/成功后复位） ── */
const confirmingArchiveId = ref("");

watch(selectedId, () => {
  confirmingArchiveId.value = "";
});

async function load() {
  loading.value = true;
  errorMessage.value = "";
  try {
    if (!projectId.value) return;
    const page = await listRepositoriesV2(projectId.value);
    repositories.value = page.items;
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

function toggleConnect() {
  showConnect.value = !showConnect.value;
}

function submitConnect() {
  void run("connect", async () => {
    await createRepositoryV2(projectId.value, {
      name: form.value.name,
      url: form.value.url,
      role: form.value.role,
      default_branch: form.value.default_branch || null,
      selected_branch: form.value.selected_branch || null,
      selected_commit: form.value.selected_commit || null,
      description: form.value.description || null
    });
    showConnect.value = false;
    form.value = {
      name: "",
      url: "",
      role: "main",
      default_branch: "",
      selected_branch: "",
      selected_commit: "",
      description: ""
    };
  });
}

/** R-03-04：确认更新采用基准 —— 采用 Observed HEAD 为新 Confirmed Commit。 */
function confirmHead(row: RepositoryV2) {
  const branch = row.selected_branch ?? row.default_branch ?? "main";
  void run(`confirm-${row.id}`, () =>
    confirmRepositoryVersion(row.id, {
      selected_branch: branch,
      selected_commit: row.head_commit_sha!,
      reason: "confirm observed head"
    })
  );
}

function verify(row: RepositoryV2) {
  void run(`verify-${row.id}`, () => verifyRepositoryV2(row.id));
}

function changeRole(row: RepositoryV2, role: string) {
  void run(`role-${row.id}`, () =>
    updateRepositoryV2(row.id, {
      revision: row.revision,
      name: row.name,
      url: row.url,
      role,
      default_branch: row.default_branch,
      description: row.description
    })
  );
}

/** V1.2 §4.4：软解绑（archive 语义，不删行不删远端仓库）；两段式确认。 */
function onArchive(row: RepositoryV2) {
  if (confirmingArchiveId.value !== row.id) {
    confirmingArchiveId.value = row.id;
    return;
  }
  confirmingArchiveId.value = "";
  void run(`archive-${row.id}`, () => disconnectRepository(row.id));
}

function statusBadgeVariant(
  status: RepositoryV2["status"]
): "success" | "warning" | "danger" | "neutral" {
  if (status === "verified") return "success";
  if (status === "unreachable") return "danger";
  if (status === "pending") return "warning";
  return "neutral";
}

function formatTime(value: string | null): string {
  return value ? new Date(value).toLocaleString() : "—";
}

function rowBranch(row: RepositoryV2): string {
  return row.selected_branch ?? row.default_branch ?? "—";
}

onMounted(load);
</script>

<template>
  <SettingsAppFrame
    :breadcrumb="settingsFrame.breadcrumb(t('s01.repositories.page.title'))"
    :title="t('s01.repositories.page.title')"
    :description="t('s01.repositories.page.description')"
    :scope-label="settingsFrame.scopeLabel.value"
    :entries="settingsFrame.navEntries.value"
    :icons="settingsFrame.navIcons"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("s01.repositories.action.refresh") }}
      </Button>
      <Button variant="primary" size="sm" @click="toggleConnect">
        <Plus :size="14" aria-hidden="true" />
        {{ t("s01.repositories.action.connect") }}
      </Button>
    </template>

    <Alert v-if="errorMessage" variant="danger">
      {{ errorMessage }}
    </Alert>

    <!-- 接入表单（可折叠；V1 无 OAuth 步骤，G-04 落地后再引入） -->
    <section v-if="showConnect" class="rm-panel" data-testid="connect-form">
      <h3 class="rm-panel__title">
        {{ t("s01.repositories.connect.title") }}
      </h3>
      <div class="rm-panel__grid">
        <label class="rm-field">
          {{ t("s01.repositories.field.name") }}
          <input
            v-model="form.name"
            data-testid="connect-name"
            class="rm-input"
            type="text"
          />
        </label>
        <label class="rm-field">
          {{ t("s01.repositories.field.url") }}
          <input
            v-model="form.url"
            data-testid="connect-url"
            class="rm-input"
            type="text"
            placeholder="git@host:group/repo.git"
          />
        </label>
        <label class="rm-field">
          {{ t("s01.repositories.field.role") }}
          <select
            v-model="form.role"
            data-testid="connect-role"
            class="rm-input"
          >
            <option v-for="role in roleOptions" :key="role" :value="role">
              {{ t(`s01.repositories.role.${role}`) }}
            </option>
          </select>
        </label>
        <label class="rm-field">
          {{ t("s01.repositories.field.defaultBranch") }}
          <input v-model="form.default_branch" class="rm-input" type="text" />
        </label>
        <label class="rm-field">
          {{ t("s01.repositories.field.selectedBranch") }}
          <input v-model="form.selected_branch" class="rm-input" type="text" />
        </label>
        <label class="rm-field">
          {{ t("s01.repositories.field.selectedCommit") }}
          <input v-model="form.selected_commit" class="rm-input" type="text" />
        </label>
      </div>
      <p class="rm-hint">
        {{ t("s01.repositories.connect.hint") }}
      </p>
      <div class="rm-actions">
        <Button
          variant="primary"
          size="sm"
          :disabled="busy === 'connect' || !form.name || !form.url"
          data-testid="connect-submit"
          @click="submitConnect"
        >
          {{ t("s01.repositories.connect.submit") }}
        </Button>
        <Button variant="ghost" size="sm" @click="toggleConnect">
          {{ t("s01.repositories.action.cancel") }}
        </Button>
      </div>
    </section>

    <!-- 概览（管理摘要，非项目状态，V1.2 §6.3）。StatCard stacked 布局无 icon 时
         不渲染 label（迁入前四卡只剩裸数字），统一 compact + icon（同 Runners 节）。 -->
    <section class="rm-stats">
      <StatCard
        :label="t('s01.repositories.stats.linked')"
        :value="String(stats.linked)"
        :icon="GitBranch"
        tone="neutral"
        layout="compact"
      />
      <StatCard
        :label="t('s01.repositories.stats.verified')"
        :value="String(stats.verified)"
        :icon="CircleCheck"
        tone="success"
        layout="compact"
      />
      <StatCard
        :label="t('s01.repositories.stats.attention')"
        :value="String(stats.attention)"
        :icon="TriangleAlert"
        :tone="stats.attention > 0 ? 'warning' : 'neutral'"
        layout="compact"
      />
      <StatCard
        :label="t('s01.repositories.stats.confirmed')"
        :value="String(stats.confirmed)"
        :icon="BadgeCheck"
        tone="info"
        layout="compact"
      />
    </section>

    <!-- 空态（V1.2 §6.3：明确传达项目仍可工作） -->
    <EmptyState
      v-if="!loading && repositories.length === 0"
      :title="t('s01.repositories.empty.title')"
      :description="t('s01.repositories.empty.description')"
      :action-label="t('s01.repositories.action.connect')"
      data-testid="repo-empty"
      @action="toggleConnect"
    />

    <!-- 列表 + 详情（V1.2 §6.1/§12：筛选行 + 仿表格式列表 + 右侧详情面板） -->
    <section v-else class="rm-layout">
      <div class="rm-main">
        <div class="rm-filter">
          <input
            v-model="search"
            class="rm-input rm-filter__search"
            type="text"
            :placeholder="t('s01.repositories.filter.searchPlaceholder')"
          />
          <select v-model="roleFilter" class="rm-input">
            <option value="">
              {{ t("s01.repositories.filter.allRoles") }}
            </option>
            <option v-for="role in roleOptions" :key="role" :value="role">
              {{ t(`s01.repositories.role.${role}`) }}
            </option>
          </select>
          <select v-model="statusFilter" class="rm-input">
            <option value="">
              {{ t("s01.repositories.filter.allStatuses") }}
            </option>
            <option
              v-for="status in statusOptions"
              :key="status"
              :value="status"
            >
              {{ t(`s01.repositories.status.${status}`) }}
            </option>
          </select>
        </div>

        <div class="rm-table" data-testid="repo-list">
          <!-- 表头用 span：保证「repo-list 的直接子 div = 数据行」契约（测试据此点首行） -->
          <span class="rm-table__head" aria-hidden="true">
            <span>{{ t("s01.repositories.list.repository") }}</span>
            <span>{{ t("s01.repositories.detail.role") }}</span>
            <span>{{ t("s01.repositories.list.branch") }}</span>
            <span>{{ t("s01.repositories.list.access") }}</span>
            <span>{{ t("s01.repositories.list.lastVerified") }}</span>
            <span />
          </span>
          <p v-if="filteredRepositories.length === 0" class="rm-table__empty">
            {{ t("s01.repositories.list.noMatch") }}
          </p>
          <div
            v-for="row in filteredRepositories"
            :key="row.id"
            class="rm-table__row"
            :class="{ 'is-selected': selectedId === row.id }"
            role="button"
            tabindex="0"
            @click="selectedId = row.id"
            @keydown.enter.prevent="selectedId = row.id"
            @keydown.space.prevent="selectedId = row.id"
          >
            <span class="rm-table__repo">
              <span class="rm-table__name-line">
                <span class="rm-table__name">{{ row.name }}</span>
                <Badge
                  v-if="primaryRepositoryId === row.id"
                  variant="accent"
                  size="sm"
                  data-testid="primary-badge"
                >
                  {{ t("s01.repositories.badge.primarySuggested") }}
                </Badge>
                <Badge v-if="hasNewHead(row)" variant="warning" size="sm">
                  {{ t("s01.repositories.badge.newHead") }}
                </Badge>
              </span>
              <span class="rm-table__url">{{ row.url }}</span>
            </span>
            <span class="rm-table__cell">{{
              t(`s01.repositories.role.${row.role}`)
            }}</span>
            <span class="rm-table__cell">{{ rowBranch(row) }}</span>
            <span class="rm-table__cell">
              <Badge :variant="statusBadgeVariant(row.status)" size="sm">
                {{ t(`s01.repositories.status.${row.status}`) }}
              </Badge>
            </span>
            <span class="rm-table__cell">{{
              formatTime(row.last_verified_at)
            }}</span>
            <span class="rm-table__go">
              <ChevronRight :size="14" aria-hidden="true" />
            </span>
          </div>
        </div>

        <p class="rm-footnote">
          {{ t("s01.repositories.list.footnote") }}
        </p>
      </div>

      <aside v-if="selected" class="rm-detail" data-testid="repo-detail">
        <header class="rm-detail__head">
          <div class="rm-detail__id">
            <h3 class="rm-detail__name">{{ selected.name }}</h3>
            <p class="rm-detail__url">{{ selected.url }}</p>
          </div>
          <button
            type="button"
            class="rm-detail__close"
            :aria-label="t('s01.repositories.detail.close')"
            data-testid="detail-close"
            @click="selectedId = ''"
          >
            <X :size="14" aria-hidden="true" />
          </button>
        </header>

        <dl class="rm-kv">
          <div class="rm-kv__row">
            <dt>{{ t("s01.repositories.detail.role") }}</dt>
            <dd>
              <select
                :value="selected.role"
                data-testid="role-select"
                class="rm-input rm-input--compact"
                @change="
                  changeRole(
                    selected!,
                    ($event.target as HTMLSelectElement).value
                  )
                "
              >
                <option v-for="role in roleOptions" :key="role" :value="role">
                  {{ t(`s01.repositories.role.${role}`) }}
                </option>
              </select>
            </dd>
          </div>
          <div class="rm-kv__row">
            <dt>{{ t("s01.repositories.detail.defaultBranch") }}</dt>
            <dd>{{ selected.default_branch ?? "—" }}</dd>
          </div>
          <div class="rm-kv__row">
            <dt>{{ t("s01.repositories.detail.observedHead") }}</dt>
            <dd class="rm-mono">{{ selected.head_commit_sha ?? "—" }}</dd>
          </div>
          <div class="rm-kv__row">
            <dt>{{ t("s01.repositories.detail.confirmedCommit") }}</dt>
            <dd class="rm-mono">
              {{
                selected.selected_commit ??
                t("s01.repositories.detail.unconfirmed")
              }}
            </dd>
          </div>
          <div class="rm-kv__row">
            <dt>{{ t("s01.repositories.detail.lastVerified") }}</dt>
            <dd>{{ formatTime(selected.last_verified_at) }}</dd>
          </div>
        </dl>

        <!-- HEAD 变化：只提示 + 人工确认（R-03-04，V1.2 §5.2/§12） -->
        <div
          v-if="hasNewHead(selected)"
          class="rm-headchange"
          data-testid="head-change"
        >
          <p class="rm-headchange__notice">
            {{ t("s01.repositories.headChange.notice") }}
          </p>
          <Button
            variant="primary"
            size="sm"
            class="rm-headchange__confirm"
            :disabled="busy === `confirm-${selected.id}`"
            data-testid="confirm-update"
            @click="confirmHead(selected)"
          >
            {{ t("s01.repositories.headChange.confirm") }}
          </Button>
        </div>

        <Alert v-if="selected.last_error_code" variant="danger" size="sm">
          {{ selected.last_error_code }}: {{ selected.last_error_detail }}
        </Alert>

        <div class="rm-actions">
          <Button
            variant="secondary"
            size="sm"
            :disabled="busy === `verify-${selected.id}`"
            data-testid="verify-btn"
            @click="verify(selected)"
          >
            {{ t("s01.repositories.action.verify") }}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            :class="{ 'is-confirming': confirmingArchiveId === selected.id }"
            :disabled="busy === `archive-${selected.id}`"
            data-testid="archive-btn"
            @click="onArchive(selected)"
          >
            {{
              confirmingArchiveId === selected.id
                ? t("s01.repositories.action.archiveConfirm")
                : t("s01.repositories.action.archive")
            }}
          </Button>
        </div>
      </aside>
    </section>
  </SettingsAppFrame>
</template>

<style scoped>
.rm-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.rm-panel__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.rm-panel__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

@media (max-width: 880px) {
  .rm-panel__grid {
    grid-template-columns: minmax(0, 1fr);
  }
}

.rm-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
}

@media (max-width: 1100px) {
  .rm-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

.rm-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 360px;
  gap: var(--space-4);
  align-items: start;
}

@media (max-width: 1440px) {
  .rm-layout {
    grid-template-columns: minmax(0, 1fr) 320px;
  }
}

@media (max-width: 1100px) {
  .rm-layout {
    grid-template-columns: minmax(0, 1fr);
  }
}

.rm-main {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.rm-filter {
  display: flex;
  gap: var(--space-2);
}

.rm-filter__search {
  flex: 1 1 auto;
  min-width: 0;
}

.rm-table {
  display: flex;
  flex-direction: column;
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow-x: auto;
}

/* 固定列宽合计 ~478px：主列过窄时不许把仓库名压成 0，横向滚动兜底 */
.rm-table__head,
.rm-table__row {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) 96px 96px 104px 150px 32px;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-4);
  min-width: 700px;
}

@media (max-width: 880px) {
  .rm-table__head,
  .rm-table__row {
    grid-template-columns: minmax(0, 1fr) 96px 104px 32px;
  }

  /* 窄屏收起分支/最近验证列（§3.2：列表优先身份与状态）；
   * 行内 span 序 = repo(1)/role(2)/branch(3)/access(4)/verified(5)/go(6) */
  .rm-table__head span:nth-child(3),
  .rm-table__head span:nth-child(5),
  .rm-table__row > span:nth-of-type(3),
  .rm-table__row > span:nth-of-type(5) {
    display: none;
  }
}

.rm-table__head {
  height: var(--size-row);
  border-bottom: 1px solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-table__row {
  min-height: var(--size-row);
  border-bottom: 1px solid var(--ws-border);
  cursor: pointer;
  transition: background-color var(--duration-fast, 120ms) ease;
}

.rm-table__row:last-child {
  border-bottom: none;
}

.rm-table__row:hover {
  background: var(--surface-sunken);
}

.rm-table__row.is-selected {
  background: var(--surface-sunken);
  box-shadow: inset 2px 0 0 var(--ws-accent);
}

.rm-table__row:focus-visible {
  outline: var(--size-focus-ring, 2px) solid var(--ws-accent);
  outline-offset: -2px;
}

.rm-table__repo {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  padding: var(--space-2) 0;
}

.rm-table__name-line {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rm-table__name {
  color: var(--ws-text-primary);
  font-weight: var(--weight-semibold);
}

.rm-table__url {
  overflow: hidden;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rm-table__cell {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-table__go {
  display: inline-flex;
  justify-content: flex-end;
  color: var(--ws-text-secondary);
}

.rm-table__empty {
  margin: 0;
  padding: var(--space-5);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.rm-footnote {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

/* 详情面板（V1.2 §6.5：低频信息收进侧栏） */
.rm-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.rm-detail__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-2);
}

.rm-detail__id {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.rm-detail__name {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.rm-detail__url {
  margin: 0;
  overflow-wrap: anywhere;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-detail__close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: var(--space-6);
  height: var(--space-6);
  padding: 0;
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--ws-text-secondary);
  cursor: pointer;
}

.rm-detail__close:hover {
  background: var(--surface-sunken);
}

.rm-detail__close:focus-visible {
  outline: var(--size-focus-ring, 2px) solid var(--ws-accent);
  outline-offset: 1px;
}

.rm-kv {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.rm-kv__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.rm-kv__row dt {
  flex: none;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-kv__row dd {
  margin: 0;
  min-width: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-align: right;
  overflow-wrap: anywhere;
}

.rm-mono {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
}

.rm-headchange {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--surface-sunken);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.rm-headchange__notice {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rm-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rm-actions .is-confirming {
  color: var(--ws-accent);
  border-color: var(--ws-accent);
}

/* 页面级表单控件（接入表单/筛选/角色切换；跟随 token，不引全局样式） */
.rm-input {
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

.rm-input::placeholder {
  color: var(--ws-text-secondary);
}

.rm-input:focus-visible {
  outline: var(--size-focus-ring, 2px) solid var(--ws-accent);
  outline-offset: 0;
}

.rm-input--compact {
  height: var(--space-5);
  padding: 0 var(--space-1);
}
</style>
