<script setup lang="ts">
/**
 * VizVersionBar —— 版本切换 / 版本对比 / 导出入口。
 *
 * 版本语义（ADR-VIZ-006）：发布版**不可变**，重新生成形成新版本而非覆盖；
 * 因此本组件只做「选择 + 对比 + 导出」，不提供编辑。
 *
 * 导出范围（ADR-VIZ-007 / 15 号安全审查）：
 * - SVG：P0 承诺（服务端纯 Node 出图）；
 * - PNG：条件支持（路径未闭合时不展示）；
 * - HTML：可选下载（不向浏览器投递，下载物经静态化检查，无脚本）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { VizArtifact } from "@/lib/viz/types";
import Badge from "@/components/ui/badge/Badge.vue";

interface Props {
  /** 当前产物。 */
  artifact: VizArtifact;
  /** 槽位历史（按版本倒序）。 */
  history?: VizArtifact[];
  /** 是否为解释性产物（explanatory 不提供发布/导出正式产物入口）。 */
  explanatory?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  history: () => [],
  explanatory: false
});

const { t } = useI18n();

const emit = defineEmits<{
  selectVersion: [artifactId: string];
  compare: [leftId: string, rightId: string];
  exportArtifact: [format: "svg" | "png" | "html"];
}>();

const versions = computed(() =>
  [props.artifact, ...props.history]
    .filter((a, index, list) => list.findIndex(x => x.id === a.id) === index)
    .sort((a, b) => b.version_no - a.version_no)
);

function onSelect(event: Event): void {
  const value = (event.target as HTMLSelectElement).value;
  if (value && value !== props.artifact.id) {
    emit("selectVersion", value);
  }
}

function onExport(format: "svg" | "png" | "html"): void {
  emit("exportArtifact", format);
}
</script>

<template>
  <div class="viz-version-bar" :data-testid="'viz-version-bar'">
    <div class="viz-version-bar__left">
      <label class="viz-version-bar__label" for="viz-version-select">
        {{ t("viz.version.label") }}
      </label>
      <select
        id="viz-version-select"
        class="viz-version-bar__select"
        :value="artifact.id"
        :data-testid="'viz-version-select'"
        @change="onSelect"
      >
        <option v-for="item in versions" :key="item.id" :value="item.id">
          {{ t("viz.version.item", { version: item.version_no }) }}
        </option>
      </select>
      <Badge v-if="artifact.state === 'stale'" variant="warning" size="sm">
        {{ t("viz.artifact.state.stale") }}
      </Badge>
    </div>

    <div class="viz-version-bar__actions">
      <button
        type="button"
        class="viz-version-bar__action"
        :disabled="versions.length < 2"
        :data-testid="'viz-compare'"
        @click="emit('compare', versions[0].id, versions[1].id)"
      >
        {{ t("viz.version.compare") }}
      </button>
      <button
        type="button"
        class="viz-version-bar__action"
        :data-testid="'viz-export-svg'"
        @click="onExport('svg')"
      >
        {{ t("viz.version.exportSvg") }}
      </button>
      <button
        v-if="!explanatory"
        type="button"
        class="viz-version-bar__action"
        :data-testid="'viz-export-html'"
        @click="onExport('html')"
      >
        {{ t("viz.version.exportHtml") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.viz-version-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.viz-version-bar__left,
.viz-version-bar__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.viz-version-bar__label {
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.viz-version-bar__select {
  height: var(--size-control-md);
  padding: 0 var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
}

.viz-version-bar__action {
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.viz-version-bar__action:disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}
</style>
