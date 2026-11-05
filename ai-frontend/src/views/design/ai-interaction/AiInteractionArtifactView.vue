<script setup lang="ts">
import { artifactFixture } from "@/components/ui/ai-interaction/fixtures";
import ArtifactResponse from "@/components/ui/ai-interaction/responses/ArtifactResponse.vue";
import { createMockSubmit } from "@/components/ui/ai-interaction/submitAdapter";

import DesignFrame from "../design-kit/DesignFrame.vue";
import CatalogPage from "./_shared/CatalogPage.vue";

/**
 * 交付帧 13 —— Artifact 响应卡。
 *
 * 预览 / 版本信息 / 差异切换 / 本地编辑四件事同卡完成：
 *   - 13 Preview：正文预览 + 版本（v1.4）+ 预览 / 版本差异切换 + 编辑入口；
 *   - 13b Edited & saved：编辑 → 保存后的本地草稿态（卡片显示「已编辑」徽标），
 *     与「提交至既有流程」是两条独立动作。
 */
const submit = createMockSubmit({ latencyMs: 300 });
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.artifact.title"
    subtitle-key="design.aiInteraction.artifact.subtitle"
  >
    <section class="artifact-page__legend">
      <p class="artifact-page__legend-item">
        <span class="artifact-page__legend-label">Version</span>
        <span class="artifact-page__legend-text">
          The summary row carries artifact type, version label, author and date;
          the read-only version stays v1.4 until a new version is published.
        </span>
      </p>
      <p class="artifact-page__legend-item">
        <span class="artifact-page__legend-label">Preview / diff</span>
        <span class="artifact-page__legend-text">
          The card toggles between rendered Markdown and the unified diff
          against the previous version; both views are read-only.
        </span>
      </p>
      <p class="artifact-page__legend-item">
        <span class="artifact-page__legend-label">Edit &amp; save</span>
        <span class="artifact-page__legend-text">
          Edit swaps the preview for a Markdown textarea; Save keeps the edited
          body inside the card and adds the Edited badge (a local draft record,
          not a new version).
        </span>
      </p>
    </section>

    <DesignFrame
      label="13 — Artifact / Preview, version, diff, edit"
      width="default"
      note="Live card: switch between Preview and Version diff in the view toggle, use Edit in the footer to replace the preview with the Markdown editor, or Accept draft / Submit to requirement review to record a separate outcome."
    >
      <ArtifactResponse
        :payload="artifactFixture"
        :submit="submit"
        timestamp="10:20"
      />
    </DesignFrame>

    <DesignFrame
      label="13b — Artifact / Edited &amp; saved (local draft)"
      width="default"
      note="Reproduce: click Edit, change the Markdown and press Save. The card keeps the edited body and shows the Edited badge; saving here is a local draft record, while Submit to requirement review is the separate workflow submission."
    >
      <ArtifactResponse
        :payload="artifactFixture"
        :submit="submit"
        timestamp="10:24"
      />
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.artifact-page__legend {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.artifact-page__legend-item {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
}

.artifact-page__legend-label {
  flex: none;
  min-width: var(--space-12);
  color: var(--ws-text-primary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.artifact-page__legend-text {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}
</style>
