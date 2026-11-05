<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { Plus, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import PageFrame from "@/components/shell/PageFrame.vue";
import { useScopeStore } from "@/api/scope";
import {
  confirmRepositoryVersion,
  createRepositoryV2,
  disconnectRepository,
  listRepositoriesV2,
  updateRepositoryV2,
  verifyRepositoryV2,
  type RepositoryV2
} from "@/api/initialization";

/**
 * S01-03 · Repository Management（长期管理页，V1.2 §6：非一次性向导）。
 *
 * 对接真实后端（pm-api pm_repositories.rs）：
 * - 四概念治理（V1.2 §5.1）：default_branch / selected_commit（Confirmed，仅
 *   人工经 update-version 写入）/ head_commit_sha（Observed，verify 回写）；
 *   `head_commit_sha !== selected_commit` = New commits detected → 只提示，
 *   人工「确认更新」（R-03-04）。
 * - Primary 无后端字段（G-03 挂账）：由 `role === "main"` 唯一性**派生**建议标记，
 *   0 或 ≥2 个 main 都显示「未指定主仓库（可选）」，不判失败（R-03-02）。
 * - 软解绑 = archive（R-03-01 词表 main/docs/service/infra；恢复入口待裁决不提供）。
 * - OAuth 授权不可用（G-04 挂账）：接入走 URL + 项目 SSH Key（§3.2.2 裁决）。
 */
const { t } = useI18n();
const route = useRoute();
const scope = useScopeStore();

const repositories = ref<RepositoryV2[]>([]);
const loading = ref(true);
const errorMessage = ref("");
const busy = ref("");
const selectedId = ref("");
const showConnect = ref(false);

const projectId = computed(() => {
  const q = route.query.project_id;
  if (typeof q === "string" && q) return q;
  return scope.currentProjectId && scope.currentProjectId !== "all"
    ? scope.currentProjectId
    : "";
});

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

const selected = computed(
  () => repositories.value.find(r => r.id === selectedId.value) ?? null
);

const roleOptions: Array<"main" | "docs" | "service" | "infra"> = [
  "main",
  "docs",
  "service",
  "infra"
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

async function load() {
  loading.value = true;
  errorMessage.value = "";
  try {
    if (!projectId.value) return;
    const page = await listRepositoriesV2(projectId.value);
    repositories.value = page.items;
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

/** V1.2 §4.4：软解绑（archive 语义，不删行不删远端仓库）。 */
function remove(row: RepositoryV2) {
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

onMounted(load);
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s01.repositories.breadcrumb.project') },
      { label: t('s01.repositories.breadcrumb.repositories') }
    ]"
    :title="t('s01.repositories.page.title')"
    :description="t('s01.repositories.page.description')"
  >
    <template #actions>
      <Button variant="ghost" size="sm" :disabled="loading" @click="load">
        <RefreshCw class="w-4 h-4" />
        {{ t("s01.repositories.action.refresh") }}
      </Button>
      <Button variant="primary" size="sm" @click="toggleConnect">
        <Plus class="w-4 h-4" />
        {{ t("s01.repositories.action.connect") }}
      </Button>
    </template>

    <Alert v-if="errorMessage" variant="danger" class="mb-4">
      {{ errorMessage }}
    </Alert>

    <!-- 接入表单（可折叠；V1 无 OAuth 步骤，G-04 落地后再引入） -->
    <section
      v-if="showConnect"
      class="rounded-lg border p-4 mb-4"
      data-testid="connect-form"
    >
      <h3 class="text-sm font-semibold mb-3">
        {{ t("s01.repositories.connect.title") }}
      </h3>
      <div class="grid gap-3 md:grid-cols-2">
        <label class="grid gap-1 text-sm">
          {{ t("s01.repositories.field.name") }}
          <input
            v-model="form.name"
            data-testid="connect-name"
            class="border rounded px-2 py-1"
            type="text"
          />
        </label>
        <label class="grid gap-1 text-sm">
          {{ t("s01.repositories.field.url") }}
          <input
            v-model="form.url"
            data-testid="connect-url"
            class="border rounded px-2 py-1"
            type="text"
            placeholder="git@host:group/repo.git"
          />
        </label>
        <label class="grid gap-1 text-sm">
          {{ t("s01.repositories.field.role") }}
          <select v-model="form.role" data-testid="connect-role" class="border rounded px-2 py-1">
            <option v-for="role in roleOptions" :key="role" :value="role">
              {{ t(`s01.repositories.role.${role}`) }}
            </option>
          </select>
        </label>
        <label class="grid gap-1 text-sm">
          {{ t("s01.repositories.field.defaultBranch") }}
          <input
            v-model="form.default_branch"
            class="border rounded px-2 py-1"
            type="text"
          />
        </label>
        <label class="grid gap-1 text-sm">
          {{ t("s01.repositories.field.selectedBranch") }}
          <input
            v-model="form.selected_branch"
            class="border rounded px-2 py-1"
            type="text"
          />
        </label>
        <label class="grid gap-1 text-sm">
          {{ t("s01.repositories.field.selectedCommit") }}
          <input
            v-model="form.selected_commit"
            class="border rounded px-2 py-1"
            type="text"
          />
        </label>
      </div>
      <p class="text-xs mt-2 opacity-70">
        {{ t("s01.repositories.connect.hint") }}
      </p>
      <div class="flex gap-2 mt-3">
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

    <!-- 概览（管理摘要，非项目状态） -->
    <section class="grid grid-cols-2 md:grid-cols-4 gap-3 mb-4">
      <div class="rounded-lg border p-3">
        <div class="text-xs opacity-70">{{ t("s01.repositories.stats.linked") }}</div>
        <div class="text-xl font-semibold">{{ stats.linked }}</div>
      </div>
      <div class="rounded-lg border p-3">
        <div class="text-xs opacity-70">{{ t("s01.repositories.stats.verified") }}</div>
        <div class="text-xl font-semibold">{{ stats.verified }}</div>
      </div>
      <div class="rounded-lg border p-3">
        <div class="text-xs opacity-70">{{ t("s01.repositories.stats.attention") }}</div>
        <div class="text-xl font-semibold">{{ stats.attention }}</div>
      </div>
      <div class="rounded-lg border p-3">
        <div class="text-xs opacity-70">{{ t("s01.repositories.stats.confirmed") }}</div>
        <div class="text-xl font-semibold">{{ stats.confirmed }}</div>
      </div>
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

    <!-- 列表 + 详情 -->
    <section v-else class="grid gap-4 lg:grid-cols-[1fr_360px]">
      <div class="rounded-lg border divide-y" data-testid="repo-list">
        <div
          v-for="row in repositories"
          :key="row.id"
          class="p-3 cursor-pointer hover:bg-accent/40"
          :class="{ 'bg-accent/60': selectedId === row.id }"
          @click="selectedId = row.id"
        >
          <div class="flex items-center gap-2 flex-wrap">
            <span class="font-medium">{{ row.name }}</span>
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
            <Badge :variant="statusBadgeVariant(row.status)" size="sm">
              {{ t(`s01.repositories.status.${row.status}`) }}
            </Badge>
            <Badge variant="neutral" size="sm">
              {{ t(`s01.repositories.role.${row.role}`) }}
            </Badge>
          </div>
          <div class="text-xs opacity-70 mt-1 truncate">{{ row.url }}</div>
        </div>
      </div>

      <aside v-if="selected" class="rounded-lg border p-4" data-testid="repo-detail">
        <h3 class="text-sm font-semibold">{{ selected.name }}</h3>
        <p class="text-xs opacity-70 break-all mt-1">{{ selected.url }}</p>

        <dl class="grid gap-2 mt-3 text-sm">
          <div class="flex justify-between gap-2">
            <dt class="opacity-70">{{ t("s01.repositories.detail.role") }}</dt>
            <dd>
              <select
                :value="selected.role"
                data-testid="role-select"
                class="border rounded px-1 py-0.5 text-sm"
                @change="changeRole(selected!, ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="role in roleOptions" :key="role" :value="role">
                  {{ t(`s01.repositories.role.${role}`) }}
                </option>
              </select>
            </dd>
          </div>
          <div class="flex justify-between gap-2">
            <dt class="opacity-70">{{ t("s01.repositories.detail.defaultBranch") }}</dt>
            <dd>{{ selected.default_branch ?? "—" }}</dd>
          </div>
          <div class="flex justify-between gap-2">
            <dt class="opacity-70">{{ t("s01.repositories.detail.observedHead") }}</dt>
            <dd class="font-mono">{{ selected.head_commit_sha ?? "—" }}</dd>
          </div>
          <div class="flex justify-between gap-2">
            <dt class="opacity-70">{{ t("s01.repositories.detail.confirmedCommit") }}</dt>
            <dd class="font-mono">
              {{ selected.selected_commit ?? t("s01.repositories.detail.unconfirmed") }}
            </dd>
          </div>
          <div class="flex justify-between gap-2">
            <dt class="opacity-70">{{ t("s01.repositories.detail.lastVerified") }}</dt>
            <dd>{{ selected.last_verified_at ?? "—" }}</dd>
          </div>
        </dl>

        <div v-if="hasNewHead(selected)" class="mt-3 rounded border p-2 text-sm" data-testid="head-change">
          <p>{{ t("s01.repositories.headChange.notice") }}</p>
          <Button
            variant="primary"
            size="sm"
            class="mt-2"
            :disabled="busy === `confirm-${selected.id}`"
            data-testid="confirm-update"
            @click="confirmHead(selected)"
          >
            {{ t("s01.repositories.headChange.confirm") }}
          </Button>
        </div>

        <div class="flex gap-2 mt-4 flex-wrap">
          <Button
            variant="ghost"
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
            :disabled="busy === `archive-${selected.id}`"
            data-testid="archive-btn"
            @click="remove(selected)"
          >
            {{ t("s01.repositories.action.archive") }}
          </Button>
        </div>
        <p v-if="selected.last_error_code" class="text-xs mt-2 text-destructive">
          {{ selected.last_error_code }}: {{ selected.last_error_detail }}
        </p>
      </aside>
    </section>
  </PageFrame>
</template>
