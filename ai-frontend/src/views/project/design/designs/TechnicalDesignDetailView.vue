<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ArrowLeft, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Input from "@/components/ui/input/Input.vue";
import { PageFrame } from "@/components/shell";import {
  createDesignRevision,
  decideDesignApproval,
  getDesign,
  linkDesignRequirement,
  listDesignFindings,
  listDesignRequirements,
  requestDesignApproval,
  submitDesignRevision,
  unlinkDesignRequirement,
  updateDesignRevisionDraft,
  type DesignApproval,
  type DesignDetail,
  type DesignRequirementLink,
  type ReviewFinding
} from "@/api/designs";
import {
  listProjectRequirements,
  type RequirementCoreEntry
} from "@/api/requirements";
import { translateError } from "@/api/base";
import DesignFrame from "../DesignFrame.vue";
import {
  formatDateTime,
  revisionMarkdown,
  revisionSummary,
  revisionTone,
  severityTone
} from "../design-contract";

/**
 * S03-03 技术设计详情（版本链 / 正文 / 需求覆盖 / 评审问题 / 写路径）。
 *
 * 数据 = GET /v1/pm/designs/{id}（设计 + 全部修订 + 审批）；
 * 选中某一修订后按需读该修订的需求关联与评审问题。
 *
 * R4/B04-02：本页接通写路径——每次编辑产生**新修订**（不可变版本链）；
 * 提交/请求审批/人工裁决/需求关联均调真实命令端点。状态一律由后端落定，
 * UI 只反映不推断：审批按钮只在 pending 审批行存在时出现（AI 主体在后端
 * require_human 闸门被 403——SC-05 前端不重复实现权限，只是不提供入口外的
 * 伪造成功态）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const designId = computed(() => String(route.params.designId ?? ""));
const detail = ref<DesignDetail | null>(null);
const selectedRevisionId = ref("");
const requirements = ref<DesignRequirementLink[]>([]);
const findings = ref<ReviewFinding[]>([]);
const loading = ref(true);
const loadError = ref(false);
const acting = ref("");
const actionError = ref("");

async function loadDetail() {
  if (!designId.value) return;
  loading.value = true;
  loadError.value = false;
  try {
    detail.value = await getDesign(designId.value);
    const revisions = detail.value.revisions;
    const current = detail.value.design.current_revision_id;
    const initial =
      revisions.find(rev => rev.id === current)?.id ?? revisions[0]?.id ?? "";
    selectedRevisionId.value = initial;
    await loadRevisionSide(current ? initial : "");
  } catch {
    loadError.value = true;
    detail.value = null;
  } finally {
    loading.value = false;
  }
}

async function loadRevisionSide(revisionId: string) {
  if (!revisionId || !designId.value) {
    requirements.value = [];
    findings.value = [];
    return;
  }
  try {
    const [reqs, founds] = await Promise.all([
      listDesignRequirements(designId.value, revisionId),
      listDesignFindings(designId.value, revisionId)
    ]);
    requirements.value = reqs ?? [];
    findings.value = founds ?? [];
  } catch {
    requirements.value = [];
    findings.value = [];
  }
}

onMounted(loadDetail);
watch(designId, loadDetail);

function selectRevision(revisionId: string) {
  selectedRevisionId.value = revisionId;
  void loadRevisionSide(revisionId);
}

const selectedRevision = computed(
  () =>
    detail.value?.revisions.find(rev => rev.id === selectedRevisionId.value) ??
    null
);

const openFindings = computed(() =>
  findings.value.filter(item => item.status === "open")
);

function goBack() {
  void router.push("/project/design/technical-designs");
}

// ───────────────────────────── 写路径（R4/B04-02）────────────────────────

/** 选中修订的 pending 审批行（人工裁决入口的显示条件）。 */
const pendingApproval = computed<DesignApproval | null>(
  () =>
    detail.value?.approvals.find(
      item =>
        item.status === "pending" &&
        item.target_id === selectedRevisionId.value
    ) ?? null
);

/** 新修订 / 编辑草稿表单（markdown 必填）。 */
const revisionFormOpen = ref<"create" | "edit" | null>(null);
const revisionForm = reactive({ markdown: "", summary: "" });
const revisionFormInvalid = computed(() => !revisionForm.markdown.trim());

function openNewRevision() {
  revisionForm.markdown = revisionMarkdown(selectedRevision.value?.content) ?? "";
  revisionForm.summary = "";
  revisionFormOpen.value = "create";
}

function openEditDraft() {
  revisionForm.markdown = revisionMarkdown(selectedRevision.value?.content) ?? "";
  revisionForm.summary = revisionSummary(selectedRevision.value?.content) ?? "";
  revisionFormOpen.value = "edit";
}

async function saveRevisionForm() {
  if (!designId.value || revisionFormInvalid.value || !revisionFormOpen.value)
    return;
  const mode = revisionFormOpen.value;
  acting.value = mode;
  actionError.value = "";
  try {
    const payload = {
      markdown: revisionForm.markdown.trim(),
      summary: revisionForm.summary.trim() || undefined
    };
    if (mode === "create") {
      const result = await createDesignRevision(designId.value, payload);
      revisionFormOpen.value = null;
      await loadDetail();
      if (result.revision) selectRevision(result.revision.id);
    } else if (selectedRevisionId.value) {
      await updateDesignRevisionDraft(
        designId.value,
        selectedRevisionId.value,
        payload
      );
      revisionFormOpen.value = null;
      await loadDetail();
    }
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

/** 提交修订（draft → pending_confirmation；内容冻结）。 */
async function submitRevision() {
  if (!designId.value || !selectedRevisionId.value) return;
  acting.value = "submit";
  actionError.value = "";
  try {
    await submitDesignRevision(designId.value, selectedRevisionId.value);
    await loadDetail();
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

/** 请求人工审批。 */
async function requestApproval() {
  if (!designId.value || !selectedRevisionId.value) return;
  acting.value = "request-approval";
  actionError.value = "";
  try {
    await requestDesignApproval(designId.value, selectedRevisionId.value);
    await loadDetail();
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

/** 人工裁决（approve/reject + 理由 + SC-06 CAS 双令牌）。 */
const decideReason = ref("");
async function decideApproval(decision: "approve" | "reject") {
  const approval = pendingApproval.value;
  if (!designId.value || !approval) return;
  if (decision === "reject" && !decideReason.value.trim()) return;
  acting.value = `decide-${decision}`;
  actionError.value = "";
  try {
    await decideDesignApproval(approval.id, {
      decision,
      reason: decideReason.value.trim() || undefined,
      expected_current_revision_id:
        detail.value?.design.current_revision_id ?? undefined,
      expected_row_revision: detail.value?.design.row_revision
    });
    decideReason.value = "";
    await loadDetail();
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

// ── 需求关联（仅 draft 修订可改——后端闸门，UI 同步收口）────────────────

const showLinkForm = ref(false);
const requirementCandidates = ref<RequirementCoreEntry[]>([]);
const linkRequirementId = ref("");
const linkFormInvalid = computed(() => !linkRequirementId.value);

async function openLinkForm() {
  showLinkForm.value = true;
  linkRequirementId.value = "";
  try {
    requirementCandidates.value =
      (await listProjectRequirements(currentProjectId())).items ?? [];
  } catch {
    requirementCandidates.value = [];
  }
}

/** 项目 id：详情行携带（避免依赖路由 query）。 */
function currentProjectId(): string {
  return detail.value?.design.project_id ?? "";
}

async function linkRequirement() {
  if (!designId.value || !selectedRevisionId.value || linkFormInvalid.value)
    return;
  acting.value = "link";
  actionError.value = "";
  try {
    await linkDesignRequirement(
      designId.value,
      selectedRevisionId.value,
      { requirement_id: linkRequirementId.value }
    );
    showLinkForm.value = false;
    await loadRevisionSide(selectedRevisionId.value);
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

async function unlinkRequirement(requirementId: string) {
  if (!designId.value || !selectedRevisionId.value) return;
  acting.value = `unlink-${requirementId}`;
  actionError.value = "";
  try {
    await unlinkDesignRequirement(
      designId.value,
      selectedRevisionId.value,
      requirementId
    );
    await loadRevisionSide(selectedRevisionId.value);
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s03.design.module') },
      {
        label: t('s03.design.page.designsTitle'),
        to: '/project/design/technical-designs'
      }
    ]"
    :title="detail?.design.title ?? t('s03.design.page.designDetailTitle')"
    :description="detail?.design.code ?? t('s03.design.page.designDetailDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="design-detail-back"
        @click="goBack"
      >
        <ArrowLeft :size="14" />
        {{ t("s03.design.common.back") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="design-detail-refresh"
        @click="loadDetail"
      >
        <RefreshCw :size="14" />
        {{ t("s03.design.common.refresh") }}
      </Button>
    </template>

    <DesignFrame>
      <Alert
        v-if="loadError"
        variant="danger"
        size="sm"
        :title="t('s03.design.common.errorTitle')"
      />

      <EmptyState
        v-else-if="!loading && !detail"
        :title="t('s03.design.designs.notFoundTitle')"
        :description="t('s03.design.designs.notFoundHint')"
      />

      <div v-else-if="detail" class="detail-layout">
        <Card class="detail-revisions" data-testid="design-revision-list">
          <template #header>
            {{ t("s03.design.detail.revisionsTitle") }}
          </template>
          <p v-if="detail.revisions.length === 0" class="detail-empty">
            {{ t("s03.design.detail.noRevisions") }}
          </p>
          <ul v-else class="revision-list">
            <li
              v-for="rev in detail.revisions"
              :key="rev.id"
              class="revision-item"
            >
              <button
                type="button"
                class="revision-item__button"
                :class="{ 'is-active': rev.id === selectedRevisionId }"
                :aria-pressed="rev.id === selectedRevisionId"
                :data-testid="`design-revision-${rev.version}`"
                @click="selectRevision(rev.id)"
              >
                <span class="revision-item__version">
                  {{
                    t("s03.design.analysis.version", { version: rev.version })
                  }}
                </span>
                <Badge :variant="revisionTone(rev.state)" size="sm">
                  {{ t(`s03.design.revisionState.${rev.state}`) }}
                </Badge>
                <span class="revision-item__meta">
                  {{ rev.created_by_name ?? rev.created_by }}
                </span>
                <span class="revision-item__meta">
                  {{ formatDateTime(rev.created_at) }}
                </span>
                <Badge v-if="rev.source === 'ai'" variant="info" size="sm">
                  {{ t("s03.design.detail.aiSource") }}
                </Badge>
              </button>
            </li>
          </ul>
        </Card>

        <div class="detail-main">
          <!-- R4/B04-02：修订动作条（状态驱动；状态一律后端权威）。 -->
          <div class="detail-actions" data-testid="design-revision-actions">
            <Button
              variant="secondary"
              size="sm"
              data-testid="design-new-revision"
              :disabled="acting !== ''"
              @click="openNewRevision"
            >
              {{ t("s03.design.detail.newRevision") }}
            </Button>
            <Button
              v-if="selectedRevision?.state === 'draft'"
              variant="secondary"
              size="sm"
              data-testid="design-edit-draft"
              :disabled="acting !== ''"
              @click="openEditDraft"
            >
              {{ t("s03.design.detail.editDraft") }}
            </Button>
            <Button
              v-if="selectedRevision?.state === 'draft'"
              variant="primary"
              size="sm"
              data-testid="design-submit-revision"
              :loading="acting === 'submit'"
              @click="submitRevision"
            >
              {{ t("s03.design.detail.submitRevision") }}
            </Button>
            <Button
              v-if="selectedRevision?.state === 'pending_confirmation' && !pendingApproval"
              variant="primary"
              size="sm"
              data-testid="design-request-approval"
              :loading="acting === 'request-approval'"
              @click="requestApproval"
            >
              {{ t("s03.design.detail.requestApproval") }}
            </Button>
          </div>

          <Alert v-if="actionError" variant="danger" size="sm">
            <span data-testid="design-action-error">{{ actionError }}</span>
          </Alert>

          <!-- 新修订 / 编辑草稿表单 -->
          <Card v-if="revisionFormOpen" class="detail-form">
            <template #header>
              {{
                revisionFormOpen === "create"
                  ? t("s03.design.detail.newRevisionTitle")
                  : t("s03.design.detail.editDraftTitle")
              }}
            </template>
            <FormField
              class="detail-form__field"
              :label="t('s03.design.detail.fieldSummary')"
            >
              <Input
                v-model="revisionForm.summary"
                data-testid="design-revision-summary-input"
              />
            </FormField>
            <FormField
              class="detail-form__field"
              :label="t('s03.design.detail.fieldMarkdown')"
            >
              <textarea
                v-model="revisionForm.markdown"
                class="detail-form__markdown"
                rows="14"
                data-testid="design-revision-markdown-input"
              />
            </FormField>
            <div class="detail-form__actions">
              <Button
                variant="primary"
                size="sm"
                :loading="acting === 'create' || acting === 'edit'"
                :disabled="revisionFormInvalid"
                data-testid="design-revision-save"
                @click="saveRevisionForm"
              >
                {{ t("s03.design.common.save") }}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                @click="revisionFormOpen = null"
              >
                {{ t("s03.design.common.cancel") }}
              </Button>
            </div>
          </Card>

          <!-- 人工审批面板（pending 审批行存在时；后端 require_human 闸门）-->
          <Card
            v-if="pendingApproval"
            class="detail-approval"
            data-testid="design-approval-panel"
          >
            <template #header>
              {{ t("s03.design.detail.approvalTitle") }}
            </template>
            <p class="detail-empty">
              {{ t("s03.design.detail.approvalHint") }}
            </p>
            <FormField
              class="detail-form__field"
              :label="t('s03.design.detail.fieldReason')"
            >
              <Input
                v-model="decideReason"
                data-testid="design-approval-reason"
              />
            </FormField>
            <div class="detail-form__actions">
              <Button
                variant="primary"
                size="sm"
                :loading="acting === 'decide-approve'"
                data-testid="design-approve"
                @click="decideApproval('approve')"
              >
                {{ t("s03.design.detail.approve") }}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                :loading="acting === 'decide-reject'"
                :disabled="!decideReason.trim()"
                data-testid="design-reject"
                @click="decideApproval('reject')"
              >
                {{ t("s03.design.detail.reject") }}
              </Button>
            </div>
          </Card>

          <Card class="detail-content">
            <template #header>
              {{ t("s03.design.detail.contentTitle") }}
            </template>
            <p
              v-if="revisionSummary(selectedRevision?.content)"
              class="detail-summary"
              data-testid="design-revision-summary"
            >
              {{ revisionSummary(selectedRevision?.content) }}
            </p>
            <pre
              v-if="revisionMarkdown(selectedRevision?.content)"
              class="detail-markdown"
              data-testid="design-revision-markdown"
              >{{ revisionMarkdown(selectedRevision?.content) }}</pre
            >
            <p v-else class="detail-empty">
              {{ t("s03.design.detail.noContent") }}
            </p>
          </Card>

          <Card class="detail-links">
            <template #header>
              {{ t("s03.design.analysis.requirementsTitle") }}
            </template>
            <!-- 关联只挂 draft 修订（后端闸门同口径）；已批准需求为缺省绑定目标。 -->
            <div
              v-if="selectedRevision?.state === 'draft'"
              class="detail-links__actions"
            >
              <Button
                variant="secondary"
                size="sm"
                data-testid="design-open-link-form"
                :disabled="acting !== ''"
                @click="openLinkForm"
              >
                {{ t("s03.design.detail.linkRequirement") }}
              </Button>
            </div>
            <div
              v-if="showLinkForm && selectedRevision?.state === 'draft'"
              class="detail-links__form"
              data-testid="design-link-form"
            >
              <select
                v-model="linkRequirementId"
                class="detail-links__select"
                data-testid="design-link-requirement-select"
              >
                <option value="">
                  {{ t("s03.design.detail.selectRequirement") }}
                </option>
                <option
                  v-for="item in requirementCandidates"
                  :key="item.id"
                  :value="item.id"
                >
                  {{ item.code }} · {{ item.title }}
                </option>
              </select>
              <Button
                variant="primary"
                size="sm"
                :loading="acting === 'link'"
                :disabled="linkFormInvalid"
                data-testid="design-link-submit"
                @click="linkRequirement"
              >
                {{ t("s03.design.detail.linkSubmit") }}
              </Button>
            </div>
            <p v-if="requirements.length === 0" class="detail-empty">
              {{ t("s03.design.analysis.noRequirements") }}
            </p>
            <ul v-else class="link-list">
              <li
                v-for="link in requirements"
                :key="link.requirement_id"
                class="link-row"
              >
                <span class="link-row__code">{{
                  link.requirement_code ?? ""
                }}</span>
                <Badge
                  v-if="link.is_current === false"
                  variant="warning"
                  size="sm"
                >
                  {{ t("s03.design.analysis.supersededRequirement") }}
                </Badge>
                <span class="link-row__meta">
                  {{
                    link.requirement_version
                      ? t("s03.design.analysis.version", {
                          version: link.requirement_version
                        })
                      : ""
                  }}
                </span>
                <Badge
                  v-if="link.requirement_revision_state"
                  variant="neutral"
                  size="sm"
                >
                  {{
                    t(
                      `s03.design.requirementState.${link.requirement_revision_state}`
                    )
                  }}
                </Badge>
                <Button
                  v-if="selectedRevision?.state === 'draft'"
                  variant="ghost"
                  size="sm"
                  :data-testid="`design-unlink-${link.requirement_code}`"
                  :disabled="acting !== ''"
                  @click="unlinkRequirement(link.requirement_id)"
                >
                  {{ t("s03.design.detail.unlink") }}
                </Button>
              </li>
            </ul>
          </Card>

          <Card class="detail-findings">
            <template #header>
              {{ t("s03.design.detail.findingsTitle") }}
            </template>
            <p v-if="findings.length === 0" class="detail-empty">
              {{ t("s03.design.detail.noFindings") }}
            </p>
            <ul v-else class="finding-list">
              <li
                v-for="item in findings"
                :key="item.id"
                class="finding-row"
                :data-testid="`design-finding-${item.id}`"
              >
                <Badge :variant="severityTone(item.severity)" size="sm">
                  {{ t(`s03.design.severity.${item.severity}`) }}
                </Badge>
                <span class="finding-row__body">{{ item.body }}</span>
                <Badge
                  :variant="item.status === 'open' ? 'warning' : 'neutral'"
                  size="sm"
                >
                  {{ t(`s03.design.findingStatus.${item.status}`) }}
                </Badge>
              </li>
            </ul>
          </Card>
        </div>
      </div>
    </DesignFrame>
  </PageFrame>
</template>

<style scoped>
.detail-layout {
  display: grid;
  grid-template-columns: minmax(220px, 280px) 1fr;
  gap: var(--space-5);
  align-items: start;
}

.detail-main {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.detail-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.revision-list,
.link-list,
.finding-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.revision-item__button {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  text-align: left;
  background: transparent;
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.revision-item__button:hover {
  background: var(--surface-base);
}

.revision-item__button:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.revision-item__button.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.revision-item__version {
  font-variant-numeric: tabular-nums;
  font-weight: var(--weight-semibold);
}

.revision-item__meta {
  color: var(--ws-text-secondary);
}

.detail-summary {
  margin-bottom: var(--space-3);
  font-size: var(--font-size-body);
  color: var(--ws-text-primary);
}

.detail-markdown {
  margin: 0;
  padding: var(--space-4);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-body);
  color: var(--ws-text-primary);
  white-space: pre-wrap;
  background: var(--surface-base);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.link-row,
.finding-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.link-row__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-primary);
}

.link-row__meta {
  color: var(--ws-text-secondary);
}

.finding-row__body {
  flex: 1;
  min-width: 200px;
  color: var(--ws-text-primary);
}
.detail-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.detail-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.detail-form__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.detail-form__markdown {
  width: 100%;
  padding: var(--space-2);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-body);
  color: var(--ws-text-primary);
  background: var(--surface-base);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.detail-form__actions {
  display: flex;
  gap: var(--space-2);
}

.detail-approval {
  border-color: var(--ws-color-warning);
}

.detail-links__actions {
  display: flex;
  margin-bottom: var(--space-2);
}

.detail-links__form {
  display: flex;
  gap: var(--space-2);
  align-items: center;
  margin-bottom: var(--space-3);
}

.detail-links__select {
  min-width: 240px;
  padding: var(--space-1) var(--space-2);
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
  background: var(--surface-base);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

</style>
