<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";
import { HttpError } from "@digua/http";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Switch from "@/components/ui/switch/Switch.vue";
import { useToast } from "@/components/ui/toast";
import {
  clearTaskPolicyOverride,
  getProjectAiExecutionPolicies,
  setProjectExecutionPolicy,
  type AiTeamExecutionPolicies,
  type AiTeamTaskPolicyOverride
} from "@/api/aiTeam";
import AiTeamFrame from "./AiTeamFrame.vue";
import { useProjectSettingsFrame } from "../shared";

/**
 * Project Setting · AI Team · Execution Policies 子页（UI-3，U-2=A 五子页
 * 最后一块；后端契约 pm-api `ai_team_policies.rs`，零迁移）。
 *
 * 分层卡：
 * - Organization（只读）：租户全局 '*' 行，值或「未设置（默认）」，由平台管理；
 * - Project：require_human_approval 开关（显式开/显式关/清除恢复继承组织，
 *   PUT 响应即写后行 = 回读）；低层 false 无法放宽高层的 true（提示文案）；
 * - Task types（只读）：租户内跨项目全局口径；
 * - Task overrides：本项目覆盖行，NULL 字段显示「继承」，清除走 DELETE
 *   （恢复全继承）。新增覆盖入口在任务详情页，V1 未提供（遗留）。
 * 403（写守卫：租户 admin 或项目 owner）→ 整页只读态（控件禁用 + 提示）；
 * 其余 4xx/5xx 常规错误提示；所有写操作成功后回读（PUT 用响应行 / DELETE
 * 重新 GET）。
 * 底部「合成规则」说明块照后端 composition 字段渲染（默认值由后端对空层
 * 实时计算，非前端硬编码）。
 */
const { t } = useI18n();
const toast = useToast();
const settingsFrame = useProjectSettingsFrame();
const projectId = settingsFrame.projectId;

const policies = ref<AiTeamExecutionPolicies | null>(null);
const loading = ref(true);
const errorMessage = ref("");
/** 403 后置为 false：本调用人无写权限，全部写控件进禁用态 */
const canManage = ref(true);
const actionError = ref("");
const savingProject = ref(false);
const clearingOverrideId = ref("");

const projectApproval = computed(
  () => policies.value?.project?.require_human_approval ?? null
);

async function load() {
  if (!projectId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  errorMessage.value = "";
  try {
    policies.value = await getProjectAiExecutionPolicies(projectId.value);
  } catch (error) {
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

/** 错误判别：403 → 整页只读态；其余常规提示 */
function actionErrorText(error: unknown, fallbackKey: string): string {
  if (error instanceof HttpError && error.status === 403) {
    canManage.value = false;
    return t("aiTeam.executionPolicies.forbidden");
  }
  return `${t(fallbackKey)}: ${
    error instanceof Error ? error.message : String(error ?? "")
  }`;
}

/** 项目层开关：true/false = 显式设置；null = 清除恢复继承组织。
 * PUT 响应即写后行（服务端回读），直接落地本地视图。 */
async function saveProject(next: boolean | null) {
  if (!projectId.value || savingProject.value || !canManage.value) return;
  savingProject.value = true;
  actionError.value = "";
  try {
    const row = await setProjectExecutionPolicy(projectId.value, next);
    if (policies.value) policies.value.project = row;
    toast.toast({
      variant: "success",
      title:
        next === null
          ? t("aiTeam.executionPolicies.cleared")
          : t("aiTeam.executionPolicies.saved")
    });
  } catch (error) {
    actionError.value = actionErrorText(
      error,
      "aiTeam.executionPolicies.saveFailed"
    );
    // 回读服务端真相（开关未翻转，保证页面与后端一致）
    await load();
  } finally {
    savingProject.value = false;
  }
}

/** 清除任务覆盖（DELETE 恢复全继承），成功后整页回读 */
async function clearOverride(row: AiTeamTaskPolicyOverride) {
  if (!projectId.value || clearingOverrideId.value || !canManage.value) return;
  clearingOverrideId.value = row.task_id;
  actionError.value = "";
  try {
    await clearTaskPolicyOverride(row.task_id);
    toast.toast({
      variant: "success",
      title: t("aiTeam.executionPolicies.overrideCleared")
    });
    await load();
  } catch (error) {
    actionError.value = actionErrorText(
      error,
      "aiTeam.executionPolicies.clearOverrideFailed"
    );
    await load();
  } finally {
    clearingOverrideId.value = "";
  }
}

function formatTime(value: string | null): string {
  return value ? new Date(value).toLocaleString() : "—";
}

function approvalLabel(value: boolean | null): string {
  if (value === null) return t("aiTeam.executionPolicies.inherit");
  return value
    ? t("aiTeam.executionPolicies.on")
    : t("aiTeam.executionPolicies.off");
}

function orgApprovalLabel(value: boolean): string {
  return value
    ? t("aiTeam.executionPolicies.orgApprovalOn")
    : t("aiTeam.executionPolicies.orgApprovalOff");
}

function capabilityLabel(value: boolean): string {
  return value
    ? t("aiTeam.executionPolicies.capabilityOn")
    : t("aiTeam.executionPolicies.capabilityOff");
}

function defaultsLabel(
  key: "auto_accept" | "auto_merge" | "auto_publish_knowledge"
): string {
  return t(`aiTeam.executionPolicies.defaultsLabel.${key}`);
}

const capabilityDefaults = computed(() => {
  const defaults = policies.value?.composition.defaults;
  if (!defaults) return [];
  return [
    { key: "auto_accept", value: defaults.auto_accept },
    { key: "auto_merge", value: defaults.auto_merge },
    { key: "auto_publish_knowledge", value: defaults.auto_publish_knowledge }
  ] as const;
});

const compositionLayers = computed(() =>
  (policies.value?.composition.layer_order ?? []).join(" → ")
);

onMounted(load);
</script>

<template>
  <AiTeamFrame :description="t('aiTeam.executionPolicies.description')">
    <template #actions>
      <Button
        variant="ghost"
        size="sm"
        :disabled="loading"
        data-testid="ai-exec-pol-refresh"
        @click="load"
      >
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("aiTeam.executionPolicies.refresh") }}
      </Button>
    </template>

    <div class="ai-exec-pol">
      <Alert
        v-if="errorMessage"
        variant="danger"
        data-testid="ai-exec-pol-error"
      >
        {{ t("aiTeam.executionPolicies.loadFailed") }}: {{ errorMessage }}
      </Alert>

      <!-- 无项目上下文：不发请求（scope 哨兵不做项目 id） -->
      <EmptyState
        v-if="!projectId"
        :title="t('aiTeam.noProject.title')"
        :description="t('aiTeam.noProject.hint')"
        data-testid="ai-exec-pol-no-project"
      />

      <template v-else>
        <Alert
          v-if="actionError"
          variant="warning"
          data-testid="ai-exec-pol-action-error"
        >
          {{ actionError }}
        </Alert>

        <!-- Organization 层（只读，平台管理） -->
        <section class="ai-exec-pol__card" data-testid="ai-exec-pol-org">
          <p class="ai-exec-pol__heading">
            {{ t("aiTeam.executionPolicies.orgHeading") }}
          </p>
          <p
            v-if="policies?.organization"
            class="ai-exec-pol__value"
            data-testid="ai-exec-pol-org-value"
          >
            {{ orgApprovalLabel(policies.organization.require_human_approval) }}
          </p>
          <p
            v-else
            class="ai-exec-pol__value"
            data-testid="ai-exec-pol-org-unset"
          >
            {{ t("aiTeam.executionPolicies.orgUnset") }}
          </p>
          <p class="ai-exec-pol__hint">
            {{ t("aiTeam.executionPolicies.orgManagedHint") }}
          </p>
          <p
            v-if="policies?.organization"
            class="ai-exec-pol__meta"
          >
            {{ t("aiTeam.executionPolicies.taskTypeColumn.updatedAt") }}:
            {{ formatTime(policies.organization.updated_at) }}
          </p>
        </section>

        <!-- Project 层（可写：开关 + 清除恢复继承） -->
        <section class="ai-exec-pol__card" data-testid="ai-exec-pol-project">
          <p class="ai-exec-pol__heading">
            {{ t("aiTeam.executionPolicies.projectHeading") }}
          </p>
          <div class="ai-exec-pol__switch-row">
            <Switch
              :model-value="projectApproval === true"
              size="sm"
              :disabled="!canManage || loading || savingProject"
              :label="t('aiTeam.executionPolicies.field.requireHumanApproval')"
              :data-testid="'ai-exec-pol-project-switch'"
              @update:model-value="next => saveProject(next)"
            />
            <span
              class="ai-exec-pol__switch-label"
              data-testid="ai-exec-pol-project-state"
            >
              {{
                projectApproval === null
                  ? t("aiTeam.executionPolicies.projectUnset")
                  : t("aiTeam.executionPolicies.field.requireHumanApproval") +
                    ": " +
                    (projectApproval
                      ? t("aiTeam.executionPolicies.on")
                      : t("aiTeam.executionPolicies.off"))
              }}
            </span>
            <Button
              v-if="policies?.project"
              variant="ghost"
              size="sm"
              :disabled="!canManage || savingProject"
              :loading="savingProject"
              data-testid="ai-exec-pol-project-clear"
              @click="saveProject(null)"
            >
              {{ t("aiTeam.executionPolicies.clearInherit") }}
            </Button>
          </div>
          <p class="ai-exec-pol__hint">
            {{ t("aiTeam.executionPolicies.projectHint") }}
          </p>
          <p
            v-if="policies?.project"
            class="ai-exec-pol__meta"
          >
            {{ t("aiTeam.executionPolicies.taskTypeColumn.updatedAt") }}:
            {{ formatTime(policies.project.updated_at) }}
          </p>
        </section>

        <!-- Task type 层（只读，租户跨项目全局） -->
        <section class="ai-exec-pol__card" data-testid="ai-exec-pol-task-types">
          <p class="ai-exec-pol__heading">
            {{ t("aiTeam.executionPolicies.taskTypesHeading") }}
          </p>
          <p class="ai-exec-pol__hint">
            {{ t("aiTeam.executionPolicies.taskTypesHint") }}
          </p>
          <p
            v-if="!policies || policies.taskTypes.length === 0"
            class="ai-exec-pol__empty"
            data-testid="ai-exec-pol-task-types-empty"
          >
            {{ t("aiTeam.executionPolicies.taskTypesEmpty") }}
          </p>
          <template v-else>
            <span class="ai-exec-pol__head" aria-hidden="true">
              <span>{{ t("aiTeam.executionPolicies.taskTypeColumn.type") }}</span>
              <span>{{
                t("aiTeam.executionPolicies.taskTypeColumn.requireHumanApproval")
              }}</span>
              <span>{{
                t("aiTeam.executionPolicies.taskTypeColumn.updatedAt")
              }}</span>
            </span>
            <div
              v-for="row in policies.taskTypes"
              :key="row.task_type"
              class="ai-exec-pol__row"
              :data-testid="`ai-exec-pol-task-type-${row.task_type}`"
            >
              <span class="ai-exec-pol__mono">{{ row.task_type }}</span>
              <span>
                <Badge
                  :variant="row.require_human_approval ? 'warning' : 'neutral'"
                  size="sm"
                >
                  {{ approvalLabel(row.require_human_approval) }}
                </Badge>
              </span>
              <span class="ai-exec-pol__cell">
                {{ formatTime(row.updated_at) }}
              </span>
            </div>
          </template>
        </section>

        <!-- 任务级覆盖（清除入口；新增入口在任务详情页，V1 未提供） -->
        <section class="ai-exec-pol__card" data-testid="ai-exec-pol-overrides">
          <p class="ai-exec-pol__heading">
            {{ t("aiTeam.executionPolicies.overridesHeading") }}
          </p>
          <p class="ai-exec-pol__hint">
            {{ t("aiTeam.executionPolicies.overridesHint") }}
          </p>
          <p
            v-if="!policies || policies.taskOverrides.length === 0"
            class="ai-exec-pol__empty"
            data-testid="ai-exec-pol-overrides-empty"
          >
            {{ t("aiTeam.executionPolicies.overridesEmpty") }}
          </p>
          <template v-else>
            <span class="ai-exec-pol__head ai-exec-pol__head--wide" aria-hidden="true">
              <span>{{ t("aiTeam.executionPolicies.overrideColumn.task") }}</span>
              <span>{{ t("aiTeam.executionPolicies.overrideColumn.autoAccept") }}</span>
              <span>{{ t("aiTeam.executionPolicies.overrideColumn.autoMerge") }}</span>
              <span>{{ t("aiTeam.executionPolicies.overrideColumn.autoPublishKnowledge") }}</span>
              <span>{{ t("aiTeam.executionPolicies.overrideColumn.requireHumanApproval") }}</span>
              <span>{{ t("aiTeam.executionPolicies.overrideColumn.updatedAt") }}</span>
              <span>{{ t("aiTeam.executionPolicies.overrideColumn.actions") }}</span>
            </span>
            <div
              v-for="row in policies.taskOverrides"
              :key="row.task_id"
              class="ai-exec-pol__row ai-exec-pol__row--wide"
              :data-testid="`ai-exec-pol-override-${row.task_id}`"
            >
              <span class="ai-exec-pol__cell ai-exec-pol__task-title">
                {{ row.task_title }}
              </span>
              <span>
                <Badge variant="neutral" size="sm">
                  {{ approvalLabel(row.auto_accept) }}
                </Badge>
              </span>
              <span>
                <Badge variant="neutral" size="sm">
                  {{ approvalLabel(row.auto_merge) }}
                </Badge>
              </span>
              <span>
                <Badge variant="neutral" size="sm">
                  {{ approvalLabel(row.auto_publish_knowledge) }}
                </Badge>
              </span>
              <span>
                <Badge
                  :variant="row.require_human_approval ? 'warning' : 'neutral'"
                  size="sm"
                >
                  {{ approvalLabel(row.require_human_approval) }}
                </Badge>
              </span>
              <span class="ai-exec-pol__cell">
                {{ formatTime(row.updated_at) }}
              </span>
              <span>
                <Button
                  variant="ghost"
                  size="sm"
                  :disabled="!canManage || clearingOverrideId === row.task_id"
                  :loading="clearingOverrideId === row.task_id"
                  :data-testid="`ai-exec-pol-override-clear`"
                  @click="clearOverride(row)"
                >
                  {{ t("aiTeam.executionPolicies.clearOverride") }}
                </Button>
              </span>
            </div>
          </template>
        </section>

        <!-- 合成规则说明（照后端 composition 字段渲染） -->
        <section
          v-if="policies"
          class="ai-exec-pol__card ai-exec-pol__card--sunken"
          data-testid="ai-exec-pol-composition"
        >
          <p class="ai-exec-pol__heading">
            {{ t("aiTeam.executionPolicies.compositionHeading") }}
          </p>
          <p class="ai-exec-pol__hint">
            {{ t("aiTeam.executionPolicies.compositionLayersLabel") }}:
            <span class="ai-exec-pol__mono">{{
              compositionLayers
            }}</span>
          </p>
          <p class="ai-exec-pol__subheading">
            {{ t("aiTeam.executionPolicies.compositionDefaultsHeading") }}
          </p>
          <ul class="ai-exec-pol__defaults">
            <li
              v-for="item in capabilityDefaults"
              :key="item.key"
              class="ai-exec-pol__default-item"
            >
              <span>{{ defaultsLabel(item.key) }}</span>
              <Badge variant="neutral" size="sm">
                {{ capabilityLabel(item.value) }}
              </Badge>
            </li>
            <li class="ai-exec-pol__default-item">
              <span>{{
                t("aiTeam.executionPolicies.defaultsLabel.requireHumanApproval")
              }}</span>
              <Badge variant="neutral" size="sm">
                {{
                  policies.composition.defaults.require_human_approval
                    ? t("aiTeam.executionPolicies.approvalRequired")
                    : t("aiTeam.executionPolicies.approvalNotRequired")
                }}
              </Badge>
            </li>
            <li class="ai-exec-pol__default-item">
              <span>{{
                t("aiTeam.executionPolicies.defaultsLabel.additionalChecks")
              }}</span>
              <span class="ai-exec-pol__cell">
                {{ policies.composition.defaults.additional_checks.length }}
              </span>
            </li>
          </ul>
          <ul class="ai-exec-pol__rules">
            <li
              v-for="rule in policies.composition.rules"
              :key="rule.key"
              class="ai-exec-pol__rule"
            >
              {{ rule.text }}
            </li>
          </ul>
        </section>
      </template>
    </div>
  </AiTeamFrame>
</template>

<style scoped>
.ai-exec-pol {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.ai-exec-pol__card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow-x: auto;
}

.ai-exec-pol__card--sunken {
  background: var(--surface-sunken);
}

.ai-exec-pol__heading {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-primary);
  font-weight: var(--weight-semibold);
}

.ai-exec-pol__subheading {
  margin: var(--space-2) 0 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
}

.ai-exec-pol__value {
  margin: 0;
  color: var(--ws-text-primary);
  font-weight: var(--weight-semibold);
}

.ai-exec-pol__hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-exec-pol__meta {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-exec-pol__empty {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-exec-pol__switch-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.ai-exec-pol__switch-label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

/* 表行网格：任务类型三列 / 覆盖七列（窄主列横向滚动兜底） */
.ai-exec-pol__head,
.ai-exec-pol__row {
  display: grid;
  grid-template-columns: minmax(160px, 1fr) 120px 170px;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) 0;
  min-width: 560px;
}

.ai-exec-pol__head--wide,
.ai-exec-pol__row--wide {
  grid-template-columns:
    minmax(180px, 1.4fr) repeat(4, minmax(88px, 0.7fr)) 170px 140px;
  min-width: 960px;
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-exec-pol__head {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-exec-pol__cell {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-exec-pol__mono {
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-exec-pol__task-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-exec-pol__defaults {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-exec-pol__default-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-exec-pol__rules {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: var(--space-2) 0 0;
  padding: var(--space-3);
  background: var(--ws-elevated);
  border-radius: var(--radius-sm);
  list-style: decimal;
}

.ai-exec-pol__rule {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}
</style>
