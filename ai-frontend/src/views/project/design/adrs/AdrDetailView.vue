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
import { PageFrame } from "@/components/shell";
import { translateError } from "@/api/base";
import {
  createAdrRevision,
  decideDesignApproval,
  getAdr,
  linkAdrDesign,
  linkAdrRequirement,
  listDesigns,
  requestAdrApproval,
  setAdrImplementationBlocking,
  submitAdrRevision,
  type AdrDetail,
  type AdrRevision,
  type DesignApproval,
  type DesignEntry
} from "@/api/designs";
import {
  listProjectRequirements,
  type RequirementCoreEntry
} from "@/api/requirements";
import DesignFrame from "../DesignFrame.vue";
import { adrTone, formatDateTime } from "../design-contract";

/**
 * S03-04 架构决策详情（决策内容 / 版本链 / 影响关系 / 写路径）。
 *
 * 数据 = GET /v1/pm/adrs/{id}（ADR + 全部修订 + 关联设计/需求 + 审批）。
 *
 * ADR 内容形状（content jsonb）：decision_question / context / options /
 * chosen_option / rationale / consequences——决策记录不可变：已提交的版本
 * 内容冻结，决策改变时新建版本并显式替代（§3.3 / §4）。
 *
 * R4/B04-02：接通写路径——修订草稿（draft 态）/提交（under_review）/请求
 * 人工确认/人工接受与拒绝（pending 审批行 + require_human 闸门）/关联
 * 设计与需求（draft 修订）/阻塞标记（真人门）。状态一律后端权威，UI 只
 * 反映不推断。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const adrId = computed(() => String(route.params.adrId ?? ""));
const detail = ref<AdrDetail | null>(null);
const selectedRevisionId = ref("");
const loading = ref(true);
const loadError = ref(false);
const acting = ref("");
const actionError = ref("");

async function load() {
  if (!adrId.value) return;
  loading.value = true;
  loadError.value = false;
  try {
    detail.value = await getAdr(adrId.value);
    const revisions = detail.value.revisions;
    const current = detail.value.adr.current_revision_id;
    selectedRevisionId.value =
      revisions.find(rev => rev.id === current)?.id ?? revisions[0]?.id ?? "";
  } catch {
    loadError.value = true;
    detail.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(adrId, load);

const selectedRevision = computed(
  () =>
    detail.value?.revisions.find(rev => rev.id === selectedRevisionId.value) ??
    null
);

const content = computed(() => {
  const raw = selectedRevision.value?.content;
  if (!raw || typeof raw !== "object") return {} as Record<string, unknown>;
  return raw as Record<string, unknown>;
});

function text(key: string): string {
  const value = content.value[key];
  return typeof value === "string" ? value : "";
}

function options(): string[] {
  const value = content.value.options;
  return Array.isArray(value) ? value.filter(v => typeof v === "string") : [];
}

function goBack() {
  void router.push("/project/design/adrs");
}

// ───────────────────────────── 写路径（R4/B04-02）────────────────────────

/** 选中修订的 pending adr_acceptance 审批行。 */
const pendingApproval = computed<DesignApproval | null>(
  () =>
    detail.value?.approvals.find(
      item =>
        item.status === "pending" &&
        item.kind === "adr_acceptance" &&
        item.target_id === selectedRevisionId.value
    ) ?? null
);

/** 新修订草稿表单（仅决策问题必填；其余选填）。 */
const revisionFormOpen = ref(false);
const revisionForm = reactive({
  decision_question: "",
  context: "",
  rationale: "",
  consequences: ""
});
const revisionFormInvalid = computed(
  () => !revisionForm.decision_question.trim()
);

function openNewRevision() {
  revisionForm.decision_question = text("decision_question");
  revisionForm.context = text("context");
  revisionForm.rationale = text("rationale");
  revisionForm.consequences = text("consequences");
  revisionFormOpen.value = true;
}

async function saveRevisionForm() {
  if (!adrId.value || revisionFormInvalid.value) return;
  acting.value = "create";
  actionError.value = "";
  try {
    const payload = {
      decision_question: revisionForm.decision_question.trim(),
      context: revisionForm.context.trim() || undefined,
      rationale: revisionForm.rationale.trim() || undefined,
      consequences: revisionForm.consequences.trim() || undefined
    };
    const result = await createAdrRevision(adrId.value, payload);
    revisionFormOpen.value = false;
    await load();
    const created = (result as { revision?: AdrRevision }).revision;
    if (created) selectedRevisionId.value = created.id;
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

/** 提交修订（draft → submitted；ADR proposed → under_review）。 */
async function submitRevision() {
  if (!adrId.value || !selectedRevisionId.value) return;
  acting.value = "submit";
  actionError.value = "";
  try {
    await submitAdrRevision(adrId.value, selectedRevisionId.value);
    await load();
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

/** 请求人工确认（under_review → 审批收件箱）。 */
async function requestApproval() {
  if (!adrId.value) return;
  acting.value = "request-approval";
  actionError.value = "";
  try {
    await requestAdrApproval(adrId.value);
    await load();
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

/** 人工接受 / 拒绝（后端 require_human 闸门；拒绝必须给理由）。 */
const decideReason = ref("");
async function decideApproval(decision: "approve" | "reject") {
  const approval = pendingApproval.value;
  if (!adrId.value || !approval) return;
  if (decision === "reject" && !decideReason.value.trim()) return;
  acting.value = `decide-${decision}`;
  actionError.value = "";
  try {
    await decideDesignApproval(approval.id, {
      decision,
      reason: decideReason.value.trim() || undefined
    });
    decideReason.value = "";
    await load();
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

/** 阻塞实施标记（真人门）。 */
async function toggleBlocking() {
  if (!adrId.value || !detail.value) return;
  acting.value = "blocking";
  actionError.value = "";
  try {
    await setAdrImplementationBlocking(
      adrId.value,
      !detail.value.adr.blockers_implementation
    );
    await load();
  } catch (error) {
    actionError.value = translateError(String(error));
  } finally {
    acting.value = "";
  }
}

// ── 关联（仅 draft 修订可改——后端闸门同口径）────────────────────────

const showLinkForm = ref(false);
const linkKind = ref<"design" | "requirement">("design");
const designs = ref<DesignEntry[]>([]);
const requirementCandidates = ref<RequirementCoreEntry[]>([]);
const linkDesignId = ref("");
const linkRequirementId = ref("");

const draftRevision = computed(
  () => detail.value?.revisions.find(rev => rev.state === "draft") ?? null
);

async function openLinkForm(kind: "design" | "requirement") {
  linkKind.value = kind;
  showLinkForm.value = true;
  linkDesignId.value = "";
  linkRequirementId.value = "";
  const projectId = detail.value?.adr.project_id ?? "";
  try {
    if (kind === "design") {
      designs.value = (await listDesigns(projectId)) ?? [];
    } else {
      requirementCandidates.value =
        (await listProjectRequirements(projectId)).items ?? [];
    }
  } catch {
    designs.value = [];
    requirementCandidates.value = [];
  }
}

async function submitLink() {
  if (
    !adrId.value ||
    !draftRevision.value ||
    (linkKind.value === "design" && !linkDesignId.value) ||
    (linkKind.value === "requirement" && !linkRequirementId.value)
  ) {
    return;
  }
  acting.value = "link";
  actionError.value = "";
  try {
    if (linkKind.value === "design") {
      // 关联挂在具体设计修订上：取该设计的最新修订（链最新的认知）。
      const designDetail = (await import("@/api/designs")).getDesign(
        linkDesignId.value
      );
      const revisions = (await designDetail).revisions;
      const latest = revisions.at(-1);
      if (!latest) {
        actionError.value = t("s03.design.adrs.noDesignRevision");
        return;
      }
      await linkAdrDesign(adrId.value, draftRevision.value.id, {
        design_id: linkDesignId.value,
        design_revision_id: latest.id
      });
    } else {
      await linkAdrRequirement(adrId.value, draftRevision.value.id, {
        requirement_id: linkRequirementId.value
      });
    }
    showLinkForm.value = false;
    await load();
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
      { label: t('s03.design.page.adrsTitle'), to: '/project/design/adrs' }
    ]"
    :title="detail?.adr.title ?? t('s03.design.page.adrDetailTitle')"
    :description="detail?.adr.code ?? t('s03.design.page.adrDetailDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="adr-detail-back"
        @click="goBack"
      >
        <ArrowLeft :size="14" />
        {{ t("s03.design.common.back") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="adr-detail-refresh"
        @click="load"
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
        :title="t('s03.design.adrs.notFoundTitle')"
        :description="t('s03.design.adrs.notFoundHint')"
      />

      <div v-else-if="detail" class="adr-detail">
        <!-- R4/B04-02：动作条（状态驱动；后端权威落定）。 -->
        <div class="adr-actions" data-testid="adr-actions">
          <Button
            variant="secondary"
            size="sm"
            data-testid="adr-new-revision"
            :disabled="acting !== ''"
            @click="openNewRevision"
          >
            {{ t("s03.design.detail.newRevision") }}
          </Button>
          <Button
            v-if="draftRevision && draftRevision.state === 'draft'"
            variant="primary"
            size="sm"
            data-testid="adr-submit-revision"
            :loading="acting === 'submit'"
            @click="submitRevision"
          >
            {{ t("s03.design.adrs.submitRevision") }}
          </Button>
          <Button
            v-if="detail.adr.status === 'under_review' && !pendingApproval"
            variant="primary"
            size="sm"
            data-testid="adr-request-approval"
            :loading="acting === 'request-approval'"
            @click="requestApproval"
          >
            {{ t("s03.design.adrs.requestApproval") }}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            data-testid="adr-toggle-blocking"
            :loading="acting === 'blocking'"
            @click="toggleBlocking"
          >
            {{
              detail.adr.blockers_implementation
                ? t("s03.design.adrs.clearBlocking")
                : t("s03.design.adrs.markBlocking")
            }}
          </Button>
          <Button
            v-if="draftRevision"
            variant="secondary"
            size="sm"
            data-testid="adr-open-link-design"
            :disabled="acting !== ''"
            @click="openLinkForm('design')"
          >
            {{ t("s03.design.adrs.linkDesign") }}
          </Button>
          <Button
            v-if="draftRevision"
            variant="secondary"
            size="sm"
            data-testid="adr-open-link-requirement"
            :disabled="acting !== ''"
            @click="openLinkForm('requirement')"
          >
            {{ t("s03.design.adrs.linkRequirement") }}
          </Button>
        </div>

        <Alert v-if="actionError" variant="danger" size="sm">
          <span data-testid="adr-action-error">{{ actionError }}</span>
        </Alert>

        <!-- 人工确认面板（pending adr_acceptance；拒绝必填理由）。 -->
        <Card
          v-if="pendingApproval"
          class="adr-approval"
          data-testid="adr-approval-panel"
        >
          <template #header>
            {{ t("s03.design.adrs.approvalTitle") }}
          </template>
          <p class="meta-muted">
            {{ t("s03.design.detail.approvalHint") }}
          </p>
          <FormField :label="t('s03.design.detail.fieldReason')">
            <Input v-model="decideReason" data-testid="adr-approval-reason" />
          </FormField>
          <div class="adr-actions">
            <Button
              variant="primary"
              size="sm"
              :loading="acting === 'decide-approve'"
              data-testid="adr-accept"
              @click="decideApproval('approve')"
            >
              {{ t("s03.design.adrs.accept") }}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              :loading="acting === 'decide-reject'"
              :disabled="!decideReason.trim()"
              data-testid="adr-reject"
              @click="decideApproval('reject')"
            >
              {{ t("s03.design.detail.reject") }}
            </Button>
          </div>
        </Card>

        <!-- 新修订草稿表单 -->
        <Card v-if="revisionFormOpen" class="adr-form">
          <template #header>
            {{ t("s03.design.detail.newRevisionTitle") }}
          </template>
          <FormField :label="t('s03.design.adrs.decisionQuestion')">
            <Input
              v-model="revisionForm.decision_question"
              data-testid="adr-revision-question"
            />
          </FormField>
          <FormField :label="t('s03.design.adrs.context')">
            <textarea
              v-model="revisionForm.context"
              class="adr-form__text"
              rows="4"
              data-testid="adr-revision-context"
            />
          </FormField>
          <FormField :label="t('s03.design.adrs.rationale')">
            <textarea
              v-model="revisionForm.rationale"
              class="adr-form__text"
              rows="3"
              data-testid="adr-revision-rationale"
            />
          </FormField>
          <FormField :label="t('s03.design.adrs.consequences')">
            <textarea
              v-model="revisionForm.consequences"
              class="adr-form__text"
              rows="3"
              data-testid="adr-revision-consequences"
            />
          </FormField>
          <div class="adr-actions">
            <Button
              variant="primary"
              size="sm"
              :loading="acting === 'create'"
              :disabled="revisionFormInvalid"
              data-testid="adr-revision-save"
              @click="saveRevisionForm"
            >
              {{ t("s03.design.common.save") }}
            </Button>
            <Button variant="ghost" size="sm" @click="revisionFormOpen = false">
              {{ t("s03.design.common.cancel") }}
            </Button>
          </div>
        </Card>

        <!-- 关联表单（仅 draft 修订；design=最新修订锚定 / requirement=当前正式版）-->
        <Card v-if="showLinkForm && draftRevision" class="adr-form" data-testid="adr-link-form">
          <template #header>
            {{
              linkKind === "design"
                ? t("s03.design.adrs.linkDesignTitle")
                : t("s03.design.adrs.linkRequirementTitle")
            }}
          </template>
          <select
            v-if="linkKind === 'design'"
            v-model="linkDesignId"
            class="adr-link-select"
            data-testid="adr-link-design-select"
          >
            <option value="">
              {{ t("s03.design.detail.selectRequirement") }}
            </option>
            <option v-for="row in designs" :key="row.id" :value="row.id">
              {{ row.code }} · {{ row.title }}
            </option>
          </select>
          <select
            v-else
            v-model="linkRequirementId"
            class="adr-link-select"
            data-testid="adr-link-requirement-select"
          >
            <option value="">
              {{ t("s03.design.detail.selectRequirement") }}
            </option>
            <option
              v-for="row in requirementCandidates"
              :key="row.id"
              :value="row.id"
            >
              {{ row.code }} · {{ row.title }}
            </option>
          </select>
          <div class="adr-actions">
            <Button
              variant="primary"
              size="sm"
              :loading="acting === 'link'"
              :disabled="
                (linkKind === 'design' && !linkDesignId) ||
                (linkKind === 'requirement' && !linkRequirementId)
              "
              data-testid="adr-link-submit"
              @click="submitLink"
            >
              {{ t("s03.design.detail.linkSubmit") }}
            </Button>
            <Button variant="ghost" size="sm" @click="showLinkForm = false">
              {{ t("s03.design.common.cancel") }}
            </Button>
          </div>
        </Card>

        <Card class="adr-meta">
          <template #header>{{ t("s03.design.adrs.metaTitle") }}</template>
          <dl class="meta-list">
            <div class="meta-row">
              <dt>{{ t("s03.design.adrs.colStatus") }}</dt>
              <dd>
                <Badge :variant="adrTone(detail.adr.status)" size="sm">
                  {{ t(`s03.design.adrStatus.${detail.adr.status}`) }}
                </Badge>
              </dd>
            </div>
            <div class="meta-row">
              <dt>{{ t("s03.design.adrs.colBlocking") }}</dt>
              <dd>
                <Badge
                  v-if="detail.adr.blockers_implementation"
                  variant="danger"
                  size="sm"
                >
                  {{ t("s03.design.adrs.blocking") }}
                </Badge>
                <span v-else class="meta-muted">
                  {{ t("s03.design.adrs.notBlocking") }}
                </span>
              </dd>
            </div>
            <div class="meta-row">
              <dt>{{ t("s03.design.adrs.colUpdated") }}</dt>
              <dd class="meta-muted">
                {{ formatDateTime(detail.adr.updated_at) }}
              </dd>
            </div>
            <div v-if="detail.adr.superseded_by_adr_id" class="meta-row">
              <dt>{{ t("s03.design.adrs.supersededBy") }}</dt>
              <dd class="meta-muted">{{ detail.adr.superseded_by_adr_id }}</dd>
            </div>
          </dl>
        </Card>

        <Card class="adr-content">
          <template #header>
            <div class="adr-content__head">
              <span>{{ t("s03.design.adrs.contentTitle") }}</span>
              <nav
                v-if="detail.revisions.length > 0"
                class="adr-revisions"
                :aria-label="t('s03.design.detail.revisionsTitle')"
              >
                <button
                  v-for="rev in detail.revisions"
                  :key="rev.id"
                  type="button"
                  class="adr-revisions__item"
                  :class="{ 'is-active': rev.id === selectedRevisionId }"
                  :aria-pressed="rev.id === selectedRevisionId"
                  :data-testid="`adr-revision-${rev.version}`"
                  @click="selectedRevisionId = rev.id"
                >
                  {{
                    t("s03.design.analysis.version", { version: rev.version })
                  }}
                </button>
              </nav>
            </div>
          </template>

          <dl class="decision-list">
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.decisionQuestion") }}</dt>
              <dd>
                {{ text("decision_question") || t("s03.design.common.none") }}
              </dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.context") }}</dt>
              <dd>{{ text("context") || t("s03.design.common.none") }}</dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.options") }}</dt>
              <dd>
                <ul v-if="options().length > 0" class="option-list">
                  <li
                    v-for="option in options()"
                    :key="option"
                    class="option-row"
                  >
                    {{ option }}
                  </li>
                </ul>
                <span v-else class="meta-muted">{{
                  t("s03.design.common.none")
                }}</span>
              </dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.chosenOption") }}</dt>
              <dd>
                {{ text("chosen_option") || t("s03.design.common.none") }}
              </dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.rationale") }}</dt>
              <dd>{{ text("rationale") || t("s03.design.common.none") }}</dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.consequences") }}</dt>
              <dd>{{ text("consequences") || t("s03.design.common.none") }}</dd>
            </div>
          </dl>
        </Card>

        <Card class="adr-links">
          <template #header>{{ t("s03.design.adrs.impactTitle") }}</template>
          <p
            v-if="
              detail.design_links.length === 0 &&
              detail.requirement_links.length === 0
            "
            class="meta-muted"
          >
            {{ t("s03.design.adrs.noImpact") }}
          </p>
          <ul v-else class="link-list">
            <li
              v-for="link in detail.design_links"
              :key="link.adr_revision_id + link.design_id"
              class="link-row"
            >
              <span class="link-row__code">{{ link.design_code ?? "" }}</span>
              <span class="link-row__meta">
                {{
                  link.design_revision_version
                    ? t("s03.design.analysis.version", {
                        version: link.design_revision_version
                      })
                    : ""
                }}
              </span>
            </li>
            <li
              v-for="link in detail.requirement_links"
              :key="link.adr_revision_id + link.requirement_id"
              class="link-row"
            >
              <span class="link-row__code">{{
                link.requirement_code ?? ""
              }}</span>
              <span class="link-row__meta">
                {{
                  link.requirement_revision_version
                    ? t("s03.design.analysis.version", {
                        version: link.requirement_revision_version
                      })
                    : ""
                }}
              </span>
            </li>
          </ul>
        </Card>
      </div>
    </DesignFrame>
  </PageFrame>
</template>

<style scoped>
.adr-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.meta-list,
.decision-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.meta-row,
.decision-row {
  display: grid;
  grid-template-columns: minmax(120px, 200px) 1fr;
  gap: var(--space-3);
  font-size: var(--font-size-caption);
}

.meta-row dt,
.decision-row dt {
  color: var(--ws-text-secondary);
}

.meta-row dd,
.decision-row dd {
  margin: 0;
  color: var(--ws-text-primary);
}

.meta-muted {
  color: var(--ws-text-secondary);
}

.adr-content__head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.adr-revisions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.adr-revisions__item {
  padding: var(--space-1) var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-secondary);
  background: transparent;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.adr-revisions__item:hover {
  color: var(--ws-text-primary);
}

.adr-revisions__item:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.adr-revisions__item.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.option-list,
.link-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.option-row {
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
}

.link-row {
  display: flex;
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
.adr-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.adr-approval,
.adr-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.adr-approval {
  border-color: var(--ws-color-warning);
}

.adr-form__text,
.adr-link-select {
  padding: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-body);
  color: var(--ws-text-primary);
  background: var(--surface-base);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.adr-form__text {
  width: 100%;
}

.adr-link-select {
  min-width: 260px;
}

</style>
