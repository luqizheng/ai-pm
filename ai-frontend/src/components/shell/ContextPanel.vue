<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  ArrowUpRight,
  Box,
  FileText,
  PanelRightClose,
  Pin,
  Sparkles
} from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import IconTooltip from "./IconTooltip.vue";
import { useAuthStore } from "@/stores/auth";
import { useShellStore } from "@/stores/shell";
import type { RelatedLink } from "./demo";

/**
 * 右上下文栏（Application Shell §2D，320px）。
 *
 * **可选且可独立关闭**——不是每页常驻。内容随当前对象变化：
 * 当前项目 → 仓库详情 → 相关文档 → 助手可用上下文。
 *
 * 底部「已固定到本页」提示对应 pin 动作（当前为静态态，不做真实固定）。
 *
 * 当前项目取自 store：切换项目时随之更新；切换过程中不显示旧项目数据
 * （Figma「07」No stale project data is displayed）。
 */

const { t } = useI18n();
const shell = useShellStore();
const auth = useAuthStore();

/** mock 预览（本地 dev / 单测）：展示设计稿样本；发布构建走真实仓库上下文 */
const mockPreview = import.meta.env.VITE_USE_MOCK === "1";

interface DemoContext {
  organization: string;
  repository: {
    visibility: string;
    defaultBranch: string;
    role: string;
    owner: string;
  };
  links: RelatedLink[];
}

const demo = ref<DemoContext | null>(null);

onMounted(async () => {
  if (!mockPreview) return;
  const module = await import("./demo");
  demo.value = {
    organization: module.demoProject.organization,
    repository: {
      visibility: module.demoRepository.visibility,
      defaultBranch: module.demoRepository.defaultBranch,
      role: module.demoRepository.role,
      owner: module.demoRepository.owner
    },
    links: module.demoRelatedLinks
  };
});

const organization = computed(
  () => demo.value?.organization ?? auth.organization ?? ""
);

const metadata = computed(() => {
  if (demo.value) {
    return [
      { key: "visibility", value: demo.value.repository.visibility },
      { key: "defaultBranch", value: demo.value.repository.defaultBranch },
      { key: "yourRole", value: demo.value.repository.role },
      { key: "owner", value: demo.value.repository.owner }
    ];
  }
  // 真实态：仓库元数据取自单一真源（shell.assistantContext）
  const context = shell.assistantContext;
  const rows: Array<{ key: string; value: string }> = [];
  if (context.defaultBranch) {
    rows.push({ key: "defaultBranch", value: context.defaultBranch });
  }
  if (context.repositoryRole) {
    rows.push({ key: "yourRole", value: context.repositoryRole });
  }
  return rows;
});

const relatedLinks = computed<RelatedLink[]>(() => demo.value?.links ?? []);

/** 当前项目（切换中为 null → 显示切换态而非旧项目） */
const currentProject = computed(() => shell.currentProject);

/**
 * 助手可用上下文的作用域串。
 *
 * 与指令栏共用 shell store 的 `assistantContext`（整合计划 Batch 1 单一
 * 真源）；切换中不回落旧项目名（Figma「07」No stale project data is
 * displayed）。
 */
const commandScope = computed(() => {
  const context = shell.assistantContext;
  const name = context.projectName ?? t("shell.app.project.switching");
  const branch = demo.value?.repository.defaultBranch || context.defaultBranch;
  return branch ? `${name} / Repository / ${branch}` : name;
});
</script>

<template>
  <aside class="shell-context" :aria-label="t('shell.app.context.title')">
    <!-- 头部：标题 + pin + 关闭 -->
    <div class="shell-context__header">
      <h2 class="shell-context__title">{{ t("shell.app.context.title") }}</h2>
      <IconTooltip :content="t('shell.app.context.pin')" side="bottom">
        <button
          type="button"
          class="shell-context__icon-button is-pinned"
          :aria-label="t('shell.app.context.pin')"
        >
          <Pin :size="16" aria-hidden="true" />
        </button>
      </IconTooltip>
      <IconTooltip :content="t('shell.app.context.close')" side="bottom">
        <button
          type="button"
          class="shell-context__icon-button"
          :aria-label="t('shell.app.context.close')"
          @click="shell.toggleContext()"
        >
          <PanelRightClose :size="16" aria-hidden="true" />
        </button>
      </IconTooltip>
    </div>

    <div class="shell-context__body">
      <!-- 当前项目 -->
      <section class="shell-context__section">
        <p class="shell-context__eyebrow">
          {{ t("shell.app.context.currentProject") }}
        </p>
        <div class="shell-context__identity">
          <span class="shell-context__project-icon" aria-hidden="true">
            <Box :size="16" />
          </span>
          <span class="shell-context__identity-text">
            <span class="shell-context__project-name">{{
              currentProject?.name ?? t("shell.app.project.switching")
            }}</span>
            <span v-if="organization" class="shell-context__project-org">
              {{ organization }}
            </span>
          </span>
        </div>
        <p class="shell-context__description">
          {{ currentProject?.description ?? t("shell.app.project.switching") }}
        </p>
      </section>

      <hr class="shell-context__divider" />

      <!-- 仓库详情 -->
      <section class="shell-context__section">
        <h3 class="shell-context__heading">
          {{ t("shell.app.context.repositoryDetails") }}
        </h3>
        <dl class="shell-context__meta">
          <div
            v-for="row in metadata"
            :key="row.key"
            class="shell-context__meta-row"
          >
            <dt class="shell-context__meta-label">
              {{ t(`shell.app.context.fields.${row.key}`) }}
            </dt>
            <dd class="shell-context__meta-value">{{ row.value }}</dd>
          </div>
        </dl>
      </section>

      <hr class="shell-context__divider" />

      <!-- 相关文档（当前无真实文档源时整段不渲染） -->
      <section v-if="relatedLinks.length" class="shell-context__section">
        <h3 class="shell-context__heading">
          {{ t("shell.app.context.related") }}
        </h3>
        <a
          v-for="link in relatedLinks"
          :key="link.id"
          class="shell-context__link"
          href="#"
          @click.prevent
        >
          <FileText :size="14" aria-hidden="true" />
          <span class="shell-context__link-text">{{ link.label }}</span>
          <ArrowUpRight :size="12" aria-hidden="true" />
        </a>
      </section>

      <!-- 助手可用上下文 -->
      <section class="shell-context__ai">
        <div class="shell-context__ai-head">
          <Sparkles :size="14" aria-hidden="true" />
          <span>{{ t("shell.app.context.aiNotice") }}</span>
        </div>
        <p class="shell-context__ai-text">
          {{ commandScope }} · {{ t("shell.app.context.noAttachment") }}
        </p>
        <Badge variant="neutral">
          {{ t("shell.app.context.readOnly") }}
        </Badge>
      </section>
    </div>

    <!-- 底部固定提示 -->
    <div class="shell-context__footer">
      <Pin :size="12" aria-hidden="true" />
      <span>{{ t("shell.app.context.pinned") }}</span>
    </div>
  </aside>
</template>

<style scoped>
.shell-context {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow-y: auto;
  background: var(--ws-sidebar);
  border-left: var(--border-1) solid var(--ws-border);
}

/* 头部 */
.shell-context__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--ws-topbar-height);
  padding: 0 var(--space-5);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.shell-context__title {
  flex: 1;
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
}

.shell-context__icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.shell-context__icon-button:hover {
  color: var(--ws-text-primary);
  background: var(--ws-hover-veil);
}

.shell-context__icon-button:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* pin 按钮为常亮态（Figma：底色 accent-surface） */
.shell-context__icon-button.is-pinned {
  color: var(--ws-text-primary);
  background: var(--ws-color-accent-surface);
}

/* 主体 */
.shell-context__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  padding: var(--space-5);
}

.shell-context__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.shell-context__eyebrow {
  margin: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.shell-context__identity {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.shell-context__project-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-lg);
  height: var(--size-control-lg);
  color: var(--ws-text-primary);
  background: var(--ws-elevated);
  border-radius: var(--radius-sm);
}

.shell-context__identity-text {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.shell-context__project-name {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
}

.shell-context__project-org {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.shell-context__description {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

.shell-context__divider {
  height: var(--border-1);
  margin: 0;
  background: var(--ws-border);
  border: none;
}

.shell-context__heading {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
}

/* 元数据表 */
.shell-context__meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.shell-context__meta-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
}

.shell-context__meta-label {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

.shell-context__meta-value {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
}

/* 相关链接 */
.shell-context__link {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  text-decoration: none;
}

.shell-context__link:hover .shell-context__link-text {
  color: var(--ws-accent);
}

.shell-context__link:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.shell-context__link-text {
  flex: 1;
}

/* 助手上下文卡 */
.shell-context__ai {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.shell-context__ai-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
}

.shell-context__ai-text {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}

/* 底部 */
.shell-context__footer {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-5);
  margin-top: auto;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
}
</style>
