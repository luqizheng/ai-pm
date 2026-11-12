<script setup lang="ts">
/**
 * VizArtifactFrame —— 产物通道的静态呈现。
 *
 * 安全口径（docs/visualization/ai-pm-viz-15-v0-6-security-review-v1.md）：
 * 产物**不执行任何脚本**——以 `<img>` 引用 blob object URL。
 * 禁止 `<object>`/`<iframe>`：`<object>` 引用 SVG 时浏览器可执行其内嵌脚本。
 *
 * 交互缺口（P0 无节点级交互）：页脚明示「交互探索在 P1 上线」，
 * 节点级信息经表格视图与溯源面板访问（UX-D11 / UX-P0-11）。
 */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { FileWarning } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { fetchArtifactSvgUrl } from "@/api/visualizations";
import { exportArtifactHtml, exportArtifactSvg } from "@/lib/viz/export";
import type { VizArtifact } from "@/lib/viz/types";
import Badge from "@/components/ui/badge/Badge.vue";
import VizStaleBar from "./VizStaleBar.vue";

interface Props {
  /** 产物元数据。 */
  artifact: VizArtifact;
  /** 是否可接受/拒绝（对话卡宿主面控制，产物内不提供操作按钮）。 */
  actionable?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  actionable: false
});

const emit = defineEmits<{
  accept: [];
  reject: [];
  openSource: [nodeId: string];
  regenerate: [];
  exportArtifact: [format: "svg" | "html"];
}>();

const { t } = useI18n();

const svgUrl = ref<string | null>(null);
const loadError = ref<string | null>(null);

/** 取产物 SVG 并转 object URL；失败则降级（不阻断页面）。 */
async function loadSvg(): Promise<void> {
  releaseUrl();
  loadError.value = null;
  try {
    svgUrl.value = await fetchArtifactSvgUrl(props.artifact.id);
  } catch {
    loadError.value = t("viz.artifact.loadFailed");
    svgUrl.value = null;
  }
}

/** 释放 object URL——否则 Blob 泄漏。 */
function releaseUrl(): void {
  if (svgUrl.value) {
    URL.revokeObjectURL(svgUrl.value);
    svgUrl.value = null;
  }
}

/** 导出产物（服务端取字节 → 下载）。失败只提示，不阻断。 */
async function handleExport(format: "svg" | "html"): Promise<void> {
  try {
    if (format === "svg") {
      await exportArtifactSvg(
        props.artifact.id,
        props.artifact.slot_key,
        props.artifact.version_no
      );
    } else {
      await exportArtifactHtml(
        props.artifact.id,
        props.artifact.slot_key,
        props.artifact.version_no
      );
    }
    emit("exportArtifact", format);
  } catch {
    loadError.value = t("viz.artifact.exportFailed");
  }
}

watch(
  () => props.artifact.id,
  () => {
    void loadSvg();
  },
  { immediate: true }
);

onBeforeUnmount(releaseUrl);

const stateLabel = computed(() =>
  t(`viz.artifact.state.${props.artifact.state}`)
);
const stateVariant = computed(() => {
  switch (props.artifact.state) {
    case "published":
      return "success" as const;
    case "stale":
      return "warning" as const;
    case "rejected":
      return "danger" as const;
    default:
      return "neutral" as const;
  }
});
const isExplanatory = computed(() => props.artifact.nature === "explanatory");
</script>

<template>
  <section class="viz-frame" :data-testid="'viz-artifact-frame'">
    <header class="viz-frame__head">
      <div class="viz-frame__title">
        <h3 class="viz-frame__name">
          {{ artifact.spec.meta.title || artifact.slot_key }}
        </h3>
        <p class="viz-frame__meta">
          {{
            t("viz.artifact.counts", {
              nodes: artifact.spec.nodes.length,
              edges: artifact.spec.edges.length
            })
          }}
          <span class="viz-frame__dot">·</span>
          {{ t("viz.artifact.version", { version: artifact.version_no }) }}
        </p>
      </div>
      <div class="viz-frame__badges">
        <Badge :variant="stateVariant" size="sm">{{ stateLabel }}</Badge>
        <Badge
          v-if="isExplanatory"
          variant="warning"
          size="sm"
          outline
          :data-testid="'viz-nature-badge'"
        >
          {{ t("viz.artifact.explanatory") }}
        </Badge>
      </div>
    </header>

    <div
      v-if="loadError"
      class="viz-frame__error"
      :data-testid="'viz-artifact-error'"
    >
      <FileWarning :size="16" aria-hidden="true" />
      <span>{{ loadError }}</span>
    </div>

    <figure v-else-if="svgUrl" class="viz-frame__canvas">
      <img
        :src="svgUrl"
        :alt="artifact.spec.meta.title || artifact.slot_key"
        class="viz-frame__image"
        :data-testid="'viz-artifact-image'"
      />
    </figure>

    <VizStaleBar
      :visible="artifact.state === 'stale'"
      @regenerate="emit('regenerate')"
    />

    <p class="viz-frame__hint" :data-testid="'viz-interaction-hint'">
      {{ t("viz.artifact.interactionHint") }}
    </p>

    <footer class="viz-frame__actions">
      <button
        type="button"
        class="viz-frame__action"
        :data-testid="'viz-export-svg'"
        @click="handleExport('svg')"
      >
        {{ t("viz.version.exportSvg") }}
      </button>
      <button
        v-if="!isExplanatory"
        type="button"
        class="viz-frame__action"
        :data-testid="'viz-export-html'"
        @click="handleExport('html')"
      >
        {{ t("viz.version.exportHtml") }}
      </button>
      <template v-if="actionable">
        <button
          type="button"
          class="viz-frame__action viz-frame__action--accept"
          :disabled="isExplanatory"
          :data-testid="'viz-accept'"
          @click="emit('accept')"
        >
          {{ t("viz.artifact.accept") }}
        </button>
        <button
          type="button"
          class="viz-frame__action viz-frame__action--reject"
          :data-testid="'viz-reject'"
          @click="emit('reject')"
        >
          {{ t("viz.artifact.reject") }}
        </button>
      </template>
    </footer>
  </section>
</template>

<style scoped>
.viz-frame {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
}

.viz-frame__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.viz-frame__title {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.viz-frame__name {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.viz-frame__meta {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.viz-frame__dot {
  margin: 0 var(--space-1);
}

.viz-frame__badges {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

.viz-frame__canvas {
  margin: 0;
  display: flex;
  justify-content: center;
  align-items: center;
  max-height: var(--size-ai-artifact-preview-height);
  overflow: auto;
}

.viz-frame__image {
  max-width: 100%;
  height: auto;
}

.viz-frame__error,
.viz-frame__stale {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-micro);
  border-radius: var(--radius-xs);
}

.viz-frame__error {
  color: var(--danger-title);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
}

.viz-frame__stale {
  color: var(--warning-strong);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
}

.viz-frame__hint {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.viz-frame__actions {
  display: flex;
  gap: var(--space-2);
}

.viz-frame__action {
  height: var(--size-control-md);
  padding: 0 var(--space-4);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  border-radius: var(--radius-xs);
  border: var(--border-1) solid var(--border-default);
  background: var(--surface-raised);
  color: var(--text-primary);
  cursor: pointer;
}

.viz-frame__action:disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}
</style>
