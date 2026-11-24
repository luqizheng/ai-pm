<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { RotateCcw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Avatar from "@/components/ui/avatar/Avatar.vue";
import Button from "@/components/ui/button/Button.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Input from "@/components/ui/input/Input.vue";
import PageFrame from "@/components/shell/PageFrame.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import { createProject } from "@/api/projects";
import { getStoredUser } from "@/api/base/token-storage";

/**
 * S01-02 · 项目创建（Project Creation）。
 *
 * 定位：建立具有明确身份、责任人与管理边界的 Project 实体（设计稿
 * docs/ui/s01/s01-02-project-creation-ux-v1-1.md，三项用户裁决 Q1~Q3 已收编）：
 *   Q1 Key = 系统建议 + 人类确认（可编辑，服务端唯一性裁决）；
 *   Q2 Owner = 默认创建人（后端 `POST /v1/pm/projects` 自动落 5 人编制，
 *      owner=创建人），改派属创建后的项目治理动作——AI 不能指定；
 *   Q3（2026-11-05 用户裁决更新）创建成功 → 直接进入项目总控台
 *      （/project/command-center）。项目创建即可工作，不存在「等待初始化」
 *      阶段；原「跳初始化总览」路径随该页一并移除。
 *
 * 边界（延后配置原则）：仓库接入、AI-PM Cognition、Codex/MCP 引导
 * 都不属于本页；创建不因它们缺省而受阻。
 *
 * V1 后端契约：`CreateProjectBody { code, name, description, customer_id }`
 * —— Additional Information（类型/业务域/预期成果/初始上下文，S01-02 §3.4
 * 可选项）等后端字段就位后随本节恢复，均不阻塞创建。
 */

const { t } = useI18n();
const router = useRouter();

const name = ref("");
const key = ref("");
const description = ref("");
const keyEditedManually = ref(false);

const nameError = ref("");
const keyError = ref("");
const submitError = ref("");
const submitting = ref(false);

const ownerName = computed(() => {
  const user = getStoredUser();
  return (
    user?.nickname || user?.username || t("s01.project-create.ownerFallback")
  );
});

/** Key 建议：名称首个词，大写字母数字，≤10 位（设计稿示例 Smark PM → SMARK） */
function suggestKey(source: string): string {
  const word = source.trim().split(/\s+/)[0] ?? "";
  return word
    .toUpperCase()
    .replace(/[^A-Z0-9]/g, "")
    .slice(0, 10);
}

function onNameInput() {
  nameError.value = "";
  if (!keyEditedManually.value) key.value = suggestKey(name.value);
}

function onKeyInput() {
  // Key 是机器标识：收敛为大写字母数字，人工编辑后停止自动跟随
  const cleaned = key.value.toUpperCase().replace(/[^A-Z0-9]/g, "");
  if (cleaned !== key.value) key.value = cleaned;
  keyEditedManually.value = true;
  keyError.value = "";
}

function resetKey() {
  keyEditedManually.value = false;
  key.value = suggestKey(name.value);
  keyError.value = "";
}

function cancel() {
  if (window.history.length > 1) {
    router.back();
    return;
  }
  void router.push("/projects");
}

function isKeyConflict(error: unknown): boolean {
  // 后端契约：重复 code → 409 `project code \`X\` already exists`（pm_lists.rs:223）
  const err = error as { status?: number; message?: string };
  if (err?.status === 409) return true;
  return /conflict|already exists|duplicate/i.test(err?.message ?? "");
}

function errorMessage(error: unknown): string {
  const err = error as { message?: string };
  return err?.message ?? "";
}

async function submit() {
  if (submitting.value) return; // 防重复提交（S01-02 §9）
  nameError.value = keyError.value = submitError.value = "";

  const trimmedName = name.value.trim();
  const trimmedKey = key.value.trim();
  if (!trimmedName) nameError.value = t("s01.project-create.nameRequired");
  if (!trimmedKey) keyError.value = t("s01.project-create.keyRequired");
  if (nameError.value || keyError.value) return;

  submitting.value = true;
  try {
    const project = await createProject({
      code: trimmedKey,
      name: trimmedName,
      description: description.value.trim() || undefined
    });
    // Q3：创建成功即工作——直接进入项目总控台，无成功停留页、无初始化等待
    await router.push({
      path: "/project/command-center",
      query: { project_id: project.id }
    });
  } catch (error) {
    if (isKeyConflict(error)) {
      keyError.value = t("s01.project-create.keyConflict");
    } else {
      const detail = errorMessage(error);
      submitError.value = detail
        ? t("s01.project-create.failedBody", { message: detail })
        : t("s01.project-create.failedTitle");
    }
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s01.project-create.breadcrumbProjects'), to: '/projects' },
      { label: t('s01.project-create.breadcrumbCreate') }
    ]"
    :title="t('s01.project-create.pageTitle')"
    :description="t('s01.project-create.pageDescription')"
  >
    <div class="pc-page">
      <div class="pc-page__columns">
        <form class="pc-page__form" @submit.prevent="submit">
          <Alert
            v-if="submitError"
            variant="danger"
            :title="t('s01.project-create.failedTitle')"
            class="pc-page__submit-error"
          >
            <p class="pc-page__submit-error-body">{{ submitError }}</p>
          </Alert>

          <section class="pc-page__card">
            <h2 class="pc-page__section-title">
              {{ t("s01.project-create.sectionBasic") }}
            </h2>
            <FormField
              :label="t('s01.project-create.nameLabel')"
              required
              :error="nameError || undefined"
              :description="t('s01.project-create.nameHelp')"
            >
              <Input
                v-model="name"
                :placeholder="t('s01.project-create.namePlaceholder')"
                :invalid="Boolean(nameError)"
                :disabled="submitting"
                @input="onNameInput"
              />
            </FormField>

            <FormField
              :label="t('s01.project-create.keyLabel')"
              required
              :error="keyError || undefined"
              :description="t('s01.project-create.keyHelp')"
            >
              <div class="pc-page__key-row">
                <Input
                  v-model="key"
                  class="pc-page__key-input"
                  :placeholder="t('s01.project-create.keyPlaceholder')"
                  :invalid="Boolean(keyError)"
                  :disabled="submitting"
                  @input="onKeyInput"
                />
                <Button
                  v-if="keyEditedManually"
                  type="button"
                  variant="tertiary"
                  size="sm"
                  :disabled="submitting"
                  @click="resetKey"
                >
                  <RotateCcw :size="13" aria-hidden="true" />
                  {{ t("s01.project-create.keyReset") }}
                </Button>
              </div>
            </FormField>

            <FormField
              :label="t('s01.project-create.descriptionLabel')"
              :description="t('s01.project-create.optionalLabel')"
            >
              <Textarea
                v-model="description"
                :placeholder="t('s01.project-create.descriptionPlaceholder')"
                :disabled="submitting"
              />
            </FormField>
          </section>

          <section class="pc-page__card">
            <h2 class="pc-page__section-title">
              {{ t("s01.project-create.sectionOwnership") }}
            </h2>
            <FormField
              :label="t('s01.project-create.ownerLabel')"
              required
              :description="t('s01.project-create.ownerHelp')"
            >
              <div class="pc-page__owner">
                <Avatar :name="ownerName" size="md" />
                <div class="pc-page__owner-id">
                  <span class="pc-page__owner-name">{{ ownerName }}</span>
                  <span class="pc-page__owner-cap">
                    {{ t("s01.project-create.ownerYou") }}
                  </span>
                </div>
              </div>
            </FormField>
          </section>

          <div class="pc-page__actions">
            <Button
              type="button"
              variant="secondary"
              :disabled="submitting"
              @click="cancel"
            >
              {{ t("s01.project-create.cancel") }}
            </Button>
            <Button type="submit" variant="primary" :loading="submitting">
              {{
                submitting
                  ? t("s01.project-create.creating")
                  : t("s01.project-create.createLabel")
              }}
            </Button>
          </div>
        </form>

        <!-- 延后配置原则：仓库 / 认知 / 引导都不在本页，也不阻塞创建 -->
        <aside class="pc-page__rail" aria-label="Guidance">
          <div class="pc-page__rail-card">
            <h3>{{ t("s01.project-create.deferredTitle") }}</h3>
            <p>{{ t("s01.project-create.deferredBody") }}</p>
            <ul>
              <li>{{ t("s01.project-create.deferredRepository") }}</li>
              <li>{{ t("s01.project-create.deferredCognition") }}</li>
              <li>{{ t("s01.project-create.deferredBootstrap") }}</li>
            </ul>
          </div>
          <div class="pc-page__rail-card">
            <h3>{{ t("s01.project-create.afterTitle") }}</h3>
            <p>{{ t("s01.project-create.afterBody") }}</p>
          </div>
        </aside>
      </div>
    </div>
  </PageFrame>
</template>

<style scoped>
.pc-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  max-width: 1120px;
}

.pc-page__columns {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-6);
  align-items: flex-start;
}

.pc-page__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  width: 100%;
  max-width: 760px;
  min-width: 0;
}

.pc-page__card {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-6);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-lg);
}

.pc-page__section-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.pc-page__key-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
}

.pc-page__key-input {
  flex: 1 1 auto;
  min-width: 0;
}

.pc-page__owner {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: var(--size-control-lg);
  padding: var(--space-2) var(--space-3);
  background: var(--surface-sunken);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.pc-page__owner-id {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.pc-page__owner-name {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.pc-page__owner-cap {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.pc-page__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
}

.pc-page__submit-error {
  margin-bottom: var(--space-2);
}

.pc-page__submit-error-body {
  margin: 2px 0 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.pc-page__rail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  width: var(--size-form-rail-width);
  flex: none;
}

.pc-page__rail-card {
  padding: var(--space-5);
  background: var(--ws-elevated);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-lg);
}

.pc-page__rail-card h3 {
  margin: 0 0 var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
}

.pc-page__rail-card p {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.pc-page__rail-card ul {
  margin: var(--space-2) 0 0;
  padding-left: var(--space-4);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

/* 壳三栏（侧栏 232 + 上下文栏 320）+ 页内双栏（表单 760 + rail 300）需 ~1700 视口。
 * DEF-ALPHA-01（2026-11-24 实测）：旧断点 1240 没算壳双栏，1280 视口下 rail 挤爆
 * flex，表单被 min-width:0 压成零宽竖排、视觉上被 rail 覆盖。修法两层：
 * ①columns 允许 wrap——宽度不足时 rail 换行到表单下方，任何视口不重叠；
 * ②断点上调到 1700——壳双栏在场时直接收掉 rail，保证表单优先拿满行宽。 */
@media (max-width: 1700px) {
  .pc-page__rail {
    display: none;
  }
}
</style>
