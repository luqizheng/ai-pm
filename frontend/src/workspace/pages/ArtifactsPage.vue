<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import PageHeader from "./PageHeader.vue";
import {
  artifacts,
  type ApprovalStatus,
  type DiffLineType,
  type TestStatus
} from "../fixtures";
import { listArtifacts, type ArtifactRecord } from "@/api/artifacts";
import { SCOPE_ALL, useScopeStore } from "@/api/scope";
import { formatDate } from "@/api/format";

const { t } = useI18n();
const router = useRouter();
const route = useRoute();
const scope = useScopeStore();

/** mock 模式用演示数据；真后端模式按项目读产物（需求修订）。单测一律走 fixtures。 */
const useMock =
  import.meta.env.VITE_USE_MOCK === "1" || import.meta.env.MODE === "test";

/* ---------------------------------------------------------- mock 分支 */

const testLabel: Record<TestStatus, string> = {
  passed: "workspace.pages.artifacts.testPassed",
  failed: "workspace.pages.artifacts.testFailed",
  running: "workspace.pages.artifacts.testRunning"
};

const testClass: Record<TestStatus, string> = {
  passed: "is-test-passed",
  failed: "is-test-failed",
  running: "is-test-running"
};

const approvalLabel: Record<ApprovalStatus, string> = {
  pending: "workspace.pages.artifacts.approvalPending",
  approved: "workspace.pages.artifacts.approvalApproved",
  rejected: "workspace.pages.artifacts.approvalRejected"
};

const approvalClass: Record<ApprovalStatus, string> = {
  pending: "is-approval-pending",
  approved: "is-approval-approved",
  rejected: "is-approval-rejected"
};

const selectedId = ref(artifacts[0]?.id ?? "");
const selected = computed(() =>
  artifacts.find(artifact => artifact.id === selectedId.value)
);
const selectedFile = ref(artifacts[0]?.files[0]?.path ?? "");
const currentFile = computed(
  () =>
    selected.value?.files.find(file => file.path === selectedFile.value) ??
    selected.value?.files[0]
);

const verdictLabel: Record<string, string> = {
  partial: "workspace.pages.artifactsExtra.verdictPartial",
  meets: "workspace.pages.artifactsExtra.verdictMeets",
  "not-met": "workspace.pages.artifactsExtra.verdictNotMet"
};

const verdictClass: Record<string, string> = {
  partial: "is-verdict-partial",
  meets: "is-verdict-meets",
  "not-met": "is-verdict-notmet"
};

const diffClass: Record<DiffLineType, string> = {
  add: "is-add",
  del: "is-del",
  ctx: "is-ctx"
};

const testResultClass: Record<string, string> = {
  passed: "is-test-passed",
  failed: "is-test-failed",
  skipped: "is-ctx"
};

function onPick(id: string) {
  selectedId.value = id;
  const artifact = artifacts.find(a => a.id === id);
  selectedFile.value = artifact?.files[0]?.path ?? "";
}

/* ------------------------------------------------------ 真后端分支 */

const apiArtifacts = ref<ArtifactRecord[]>([]);
const selectedApiId = ref("");
const selectedApi = computed(() =>
  apiArtifacts.value.find(item => item.id === selectedApiId.value)
);

const stateLabel: Record<string, string> = {
  draft: "workspace.pages.artifacts.stateDraft",
  approved: "workspace.pages.artifacts.stateApproved",
  superseded: "workspace.pages.artifacts.stateSuperseded"
};

const stateClass: Record<string, string> = {
  draft: "is-state-draft",
  approved: "is-state-approved",
  superseded: "is-state-superseded"
};

function stateText(state: string): string {
  const key = stateLabel[state];
  return key ? String(t(key)) : state;
}

/** 项目来源：显式 scope 选择 > 路由里是真实 UUID 时用它；否则提示先选项目。 */
const projectId = computed<string | undefined>(() => {
  if (scope.projectIdParam) return scope.projectIdParam;
  const param = String(route.params.projectId ?? "");
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      param
    )
  ) {
    return param;
  }
  return undefined;
});

const selectedProjectId = computed({
  get: () =>
    scope.currentProjectId === SCOPE_ALL ? "" : scope.currentProjectId,
  set: (value: string) => {
    scope.currentProjectId = value || SCOPE_ALL;
  }
});

async function loadArtifacts() {
  if (useMock) return;
  failed.value = false;
  if (!projectId.value) {
    apiArtifacts.value = [];
    selectedApiId.value = "";
    return;
  }
  loading.value = true;
  try {
    const page = await listArtifacts({
      project_id: projectId.value,
      page: 1,
      size: 200
    });
    apiArtifacts.value = page.items ?? [];
    selectedApiId.value = apiArtifacts.value[0]?.id ?? "";
  } catch {
    failed.value = true;
    apiArtifacts.value = [];
  } finally {
    loading.value = false;
  }
}

const loading = ref(false);
const failed = ref(false);
let initialised = false;

onMounted(async () => {
  if (useMock) return;
  try {
    await scope.ensureLoaded();
  } catch {
    failed.value = true;
    return;
  }
  initialised = true;
  await loadArtifacts();
});

watch(projectId, () => {
  if (!useMock && initialised) void loadArtifacts();
});
</script>

<template>
  <div class="ws-page ws-artifacts">
    <PageHeader
      :title="t('workspace.pages.artifacts.title')"
      :subtitle="t('workspace.pages.artifacts.subtitle')"
    />

    <template v-if="useMock">
      <!-- V1.2：主从布局——左产物清单，右审查详情 -->
      <div class="ws-artifacts-body">
        <aside class="ws-card ws-artifact-list" aria-label="artifacts">
          <button
            v-for="artifact in artifacts"
            :key="artifact.id"
            type="button"
            class="ws-artifact-row ws-focusable"
            :class="{ 'is-selected': selectedId === artifact.id }"
            data-test="artifact-item"
            @click="onPick(artifact.id)"
          >
            <span class="ws-artifact-name">{{ artifact.name }}</span>
            <span class="ws-artifact-version">{{ artifact.version }}</span>
            <span class="ws-badge" :class="testClass[artifact.testStatus]">
              {{ t(testLabel[artifact.testStatus]) }}
            </span>
          </button>
        </aside>

        <article v-if="selected" class="ws-artifact-detail">
          <!-- V1.2：整体判定横幅——「部分检查通过」≠「符合验收标准」 -->
          <div
            class="ws-verdict"
            :class="verdictClass[selected.verdict]"
            data-test="artifact-verdict"
            role="status"
          >
            {{ t(verdictLabel[selected.verdict]) }}
          </div>

          <section class="ws-card ws-detail-card">
            <div class="ws-detail-head">
              <div class="ws-detail-id">
                <span class="ws-detail-name">{{ selected.name }}</span>
                <span class="ws-detail-version">{{ selected.version }}</span>
              </div>
              <div class="ws-detail-badges">
                <span class="ws-badge" :class="testClass[selected.testStatus]">
                  {{ t(testLabel[selected.testStatus]) }}
                </span>
                <span
                  class="ws-badge"
                  :class="approvalClass[selected.approval]"
                >
                  {{ t(approvalLabel[selected.approval]) }}
                </span>
              </div>
            </div>
            <div class="ws-detail-meta">
              <span>{{ selected.type }}</span>
              <span>{{ selected.owner }}</span>
              <span>{{ selected.updatedAt }}</span>
              <span>
                {{ t("workspace.pages.artifactsExtra.linkedTask") }}：{{
                  selected.linkedTask
                }}
              </span>
            </div>

            <!-- 文件 + 统一 Diff + 行级评论 -->
            <div class="ws-detail-block">
              <h3 class="ws-detail-label">
                {{ t("workspace.pages.artifactsExtra.files") }} ·
                {{ t("workspace.pages.artifactsExtra.diffLabel") }}
              </h3>
              <div class="ws-file-tabs">
                <button
                  v-for="file in selected.files"
                  :key="file.path"
                  type="button"
                  class="ws-file-tab ws-focusable"
                  :class="{ 'is-active': currentFile?.path === file.path }"
                  data-test="artifact-file"
                  @click="selectedFile = file.path"
                >
                  {{ file.path }}
                </button>
              </div>
              <div v-if="currentFile" class="ws-diff" data-test="artifact-diff">
                <div
                  v-for="(line, i) in currentFile.diff"
                  :key="i"
                  class="ws-diff-line"
                  :class="diffClass[line.type]"
                >
                  <span class="ws-diff-sign">
                    {{
                      line.type === "add"
                        ? "+"
                        : line.type === "del"
                          ? "-"
                          : " "
                    }}
                  </span>
                  <span class="ws-diff-text">{{ line.text }}</span>
                </div>
              </div>
              <div
                v-if="currentFile?.comments.length"
                class="ws-comments"
                data-test="artifact-comments"
              >
                <h4 class="ws-detail-label">
                  {{ t("workspace.pages.artifactsExtra.comments") }}
                </h4>
                <div
                  v-for="(comment, i) in currentFile.comments"
                  :key="i"
                  class="ws-comment"
                >
                  <span class="ws-comment-line">
                    {{ t("workspace.pages.artifactsExtra.lineLabel") }}
                    {{ comment.line }}
                  </span>
                  <span class="ws-comment-author">{{ comment.author }}</span>
                  <span class="ws-comment-text">{{ comment.text }}</span>
                </div>
              </div>
            </div>

            <!-- 测试报告 -->
            <div class="ws-detail-block">
              <h3 class="ws-detail-label">
                {{ t("workspace.pages.artifactsExtra.testReport") }}
              </h3>
              <ul class="ws-tests" data-test="artifact-tests">
                <li
                  v-for="(item, i) in selected.testReport"
                  :key="i"
                  class="ws-test-row"
                >
                  <span class="ws-test-name">{{ item.name }}</span>
                  <span class="ws-badge" :class="testResultClass[item.result]">
                    {{ item.result }}
                  </span>
                </li>
              </ul>
            </div>

            <!-- 需求验收标准核对 -->
            <div class="ws-detail-block">
              <h3 class="ws-detail-label">
                {{ t("workspace.pages.artifactsExtra.acceptanceLabel") }}
              </h3>
              <ul class="ws-criteria" data-test="artifact-criteria">
                <li
                  v-for="(item, i) in selected.acceptanceCriteria"
                  :key="i"
                  class="ws-criterion"
                >
                  <span
                    class="ws-criterion-mark"
                    :class="item.passed ? 'is-test-passed' : 'is-test-failed'"
                    aria-hidden="true"
                  >
                    {{ item.passed ? "✓" : "✗" }}
                  </span>
                  <span class="ws-criterion-text">{{ item.text }}</span>
                </li>
              </ul>
            </div>
          </section>
        </article>
      </div>
    </template>

    <template v-else>
      <div class="ws-toolbar">
        <label class="ws-toolbar-label" for="artifacts-project">
          {{ t("workspace.pages.artifacts.project") }}
        </label>
        <select
          id="artifacts-project"
          v-model="selectedProjectId"
          class="ws-toolbar-select"
        >
          <option value="">
            {{ t("workspace.pages.artifacts.pickProjectShort") }}
          </option>
          <option v-for="p in scope.projects" :key="p.id" :value="p.id">
            {{ p.name }}
          </option>
        </select>
      </div>
      <p v-if="!projectId" class="ws-hint" data-test="artifacts-pick-project">
        {{ t("workspace.pages.artifacts.pickProject") }}
      </p>
      <p v-else-if="failed" class="ws-hint" data-test="artifacts-failed">
        {{ t("workspace.pages.artifacts.loadFailed") }}
      </p>
      <p
        v-else-if="!loading && !apiArtifacts.length"
        class="ws-hint"
        data-test="artifacts-empty"
      >
        {{ t("workspace.pages.artifacts.empty") }}
      </p>
      <div v-else class="ws-artifacts-body">
        <aside class="ws-card ws-artifact-list" aria-label="artifacts">
          <button
            v-for="item in apiArtifacts"
            :key="item.id"
            type="button"
            class="ws-artifact-row ws-focusable"
            :class="{ 'is-selected': selectedApiId === item.id }"
            data-test="artifact-row"
            @click="selectedApiId = item.id"
          >
            <span class="ws-artifact-name">{{ item.requirement_title }}</span>
            <span class="ws-artifact-version">v{{ item.version }}</span>
            <span class="ws-badge" :class="stateClass[item.state]">
              {{ stateText(item.state) }}
            </span>
          </button>
        </aside>

        <article
          v-if="selectedApi"
          class="ws-artifact-detail"
          data-test="artifact-detail"
        >
          <section class="ws-card ws-detail-card">
            <div class="ws-detail-head">
              <div class="ws-detail-id">
                <span class="ws-detail-name">{{
                  selectedApi.requirement_title
                }}</span>
                <span class="ws-detail-version"
                  >v{{ selectedApi.version }} ·
                  {{ selectedApi.requirement_code }}</span
                >
              </div>
              <div class="ws-detail-badges">
                <span class="ws-badge" :class="stateClass[selectedApi.state]">
                  {{ stateText(selectedApi.state) }}
                </span>
              </div>
            </div>
            <div class="ws-detail-meta">
              <span>
                {{ t("workspace.pages.artifacts.hashLabel") }}：{{
                  selectedApi.content_hash.slice(0, 12)
                }}
              </span>
              <span>
                {{ t("workspace.pages.artifacts.approvedByLabel") }}：{{
                  selectedApi.approved_by
                    ? t("workspace.pages.artifacts.approvalApproved")
                    : t("workspace.pages.artifacts.noApprover")
                }}
              </span>
              <span>{{ formatDate(selectedApi.created_at) }}</span>
            </div>
            <p class="ws-hint" data-test="artifact-diff-pending">
              {{ t("workspace.pages.artifacts.diffPending") }}
            </p>
          </section>
        </article>
      </div>
    </template>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 1200px;
}

.ws-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-toolbar-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-toolbar-select {
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-surface);
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
}

.ws-hint {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-artifacts-body {
  display: grid;
  grid-template-columns: 300px minmax(0, 1fr);
  gap: 12px;
  align-items: start;
}

.ws-artifact-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px;
}

.ws-artifact-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 40px;
  padding: 4px 8px;
  border: none;
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
  cursor: pointer;
  text-align: left;
}

.ws-artifact-row:hover {
  background: var(--ws-elevated);
}

/* 紫色克制：选中态用 AI 紫 */
.ws-artifact-row.is-selected {
  background: var(--ws-elevated);
  box-shadow: var(--ws-focus-ring);
}

.ws-artifact-name {
  flex: 1;
  min-width: 0;
  font-weight: 600;
}

.ws-artifact-version {
  flex: none;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.ws-badge {
  flex: none;
  font-size: var(--ws-font-size-caption);
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid currentcolor;
}

/* 紫色克制：测试/审批状态用语义色 */
.ws-badge.is-test-passed,
.ws-badge.is-approval-approved,
.ws-badge.is-state-approved {
  color: var(--ws-color-success-text);
}

.ws-badge.is-test-failed,
.ws-badge.is-approval-rejected {
  color: var(--ws-color-danger-text);
}

.ws-badge.is-test-running,
.ws-badge.is-approval-pending,
.ws-badge.is-state-draft {
  color: var(--ws-color-warning-text);
}

.ws-badge.is-state-superseded {
  color: var(--ws-text-secondary);
}

.ws-badge.is-ctx {
  color: var(--ws-text-secondary);
}

/* V1.2：整体判定横幅 */
.ws-verdict {
  padding: 10px 14px;
  border: 1px solid currentcolor;
  border-radius: var(--ws-radius-dialog);
  font-weight: 600;
  font-size: var(--ws-font-size-body);
}

.ws-verdict.is-verdict-partial {
  color: var(--ws-color-warning-text);
}

.ws-verdict.is-verdict-meets {
  color: var(--ws-color-success-text);
}

.ws-verdict.is-verdict-notmet {
  color: var(--ws-color-danger-text);
}

.ws-detail-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
}

.ws-detail-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.ws-detail-id {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.ws-detail-name {
  font-size: var(--ws-font-size-section);
  font-weight: 600;
}

.ws-detail-version {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.ws-detail-badges {
  display: flex;
  gap: 8px;
}

.ws-detail-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-detail-block {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.ws-detail-label {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  font-weight: 600;
  color: var(--ws-text-secondary);
}

.ws-file-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.ws-file-tab {
  height: 26px;
  padding: 0 10px;
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: transparent;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  font-family: var(--font-mono, monospace);
  cursor: pointer;
}

.ws-file-tab.is-active {
  background: var(--ws-elevated);
  color: var(--ws-accent);
}

.ws-diff {
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow: hidden;
  font-family: var(--font-mono, monospace);
  font-size: var(--ws-font-size-caption);
}

.ws-diff-line {
  display: flex;
  gap: 8px;
  padding: 2px 10px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.ws-diff-sign {
  flex: none;
  width: 12px;
}

/* Diff 语义色：增绿删红中性灰 */
.ws-diff-line.is-add {
  background: color-mix(in srgb, var(--ws-color-success) 12%, transparent);
  color: var(--ws-color-success-text);
}

.ws-diff-line.is-del {
  background: color-mix(in srgb, var(--ws-color-danger) 10%, transparent);
  color: var(--ws-color-danger-text);
}

.ws-diff-line.is-ctx {
  color: var(--ws-text-secondary);
}

.ws-comments {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.ws-comment {
  display: flex;
  gap: 8px;
  padding: 8px 10px;
  border: 1px solid var(--ws-border);
  border-left: 2px solid var(--ws-accent);
  border-radius: var(--ws-radius-card);
  font-size: var(--ws-font-size-caption);
}

.ws-comment-line {
  flex: none;
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.ws-comment-author {
  flex: none;
  font-weight: 600;
}

.ws-comment-text {
  flex: 1;
  min-width: 0;
  line-height: 1.6;
}

.ws-tests,
.ws-criteria {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.ws-test-row,
.ws-criterion {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 32px;
  border-bottom: 1px solid var(--ws-border);
  font-size: var(--ws-font-size-caption);
}

.ws-test-row:last-child,
.ws-criterion:last-child {
  border-bottom: none;
}

.ws-test-name,
.ws-criterion-text {
  flex: 1;
  min-width: 0;
}

.ws-criterion-mark {
  flex: none;
  font-weight: 700;
}
</style>
