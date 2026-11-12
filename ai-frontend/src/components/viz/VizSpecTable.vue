<script setup lang="ts">
/**
 * VizSpecTable —— 降级通道，**兼作 P0 的节点导航通道**。
 *
 * P0 产物是静态图（无节点级交互），因此节点级信息经本表访问：
 * 点击行 → 打开溯源面板 / 跳转原文档（UX-D12）。
 *
 * 安全口径：只渲染**裁剪后**的 spec（服务端已完成权限过滤），
 * 前端不做二次隐藏（前端隐藏不能替代服务端过滤）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { VizEdge, VizNode, VizSpec } from "@/lib/viz/types";
import Badge from "@/components/ui/badge/Badge.vue";

interface Props {
  spec: VizSpec;
  /** 点击节点（打开溯源面板）。 */
  onSelectNode?: (node: VizNode) => void;
}

const props = defineProps<Props>();

const { t } = useI18n();

const emit = defineEmits<{ selectNode: [node: VizNode] }>();

/** 节点表：id / 标签 / 类型 / 状态 / 溯源。 */
const nodeRows = computed(() =>
  props.spec.nodes.map(node => ({
    node,
    unlinked: !node.source_ref
  }))
);

/** 边表：关系 / 起点 / 终点 / 直接或推导。 */
const edgeRows = computed(() =>
  props.spec.edges.map(edge => ({
    edge,
    derived: Boolean(edge.derived && edge.derived.direct === false)
  }))
);

function select(node: VizNode): void {
  emit("selectNode", node);
}
</script>

<template>
  <div class="viz-table" :data-testid="'viz-spec-table'">
    <section class="viz-table__block">
      <h4 class="viz-table__heading">
        {{ t("viz.table.nodes", { count: nodeRows.length }) }}
      </h4>
      <table class="viz-table__table">
        <thead>
          <tr>
            <th scope="col">{{ t("viz.table.column.label") }}</th>
            <th scope="col">{{ t("viz.table.column.kind") }}</th>
            <th scope="col">{{ t("viz.table.column.status") }}</th>
            <th scope="col">{{ t("viz.table.column.source") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="row in nodeRows"
            :key="row.node.id"
            class="viz-table__row"
            :data-testid="'viz-node-row'"
            tabindex="0"
            @click="select(row.node)"
            @keydown.enter="select(row.node)"
          >
            <td class="viz-table__cell viz-table__cell--label">
              {{ row.node.label }}
            </td>
            <td class="viz-table__cell">{{ row.node.kind }}</td>
            <td class="viz-table__cell">
              <span v-if="row.node.status">{{ row.node.status }}</span>
              <span v-else class="viz-table__muted">—</span>
            </td>
            <td class="viz-table__cell">
              <Badge v-if="row.unlinked" variant="neutral" size="sm" outline>
                {{ t("viz.table.unlinked") }}
              </Badge>
              <span v-else-if="row.node.source_ref">
                {{ row.node.source_ref.domain }}/{{ row.node.source_ref.type }}
              </span>
              <span v-else class="viz-table__muted">—</span>
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <section v-if="edgeRows.length" class="viz-table__block">
      <h4 class="viz-table__heading">
        {{ t("viz.table.edges", { count: edgeRows.length }) }}
      </h4>
      <table class="viz-table__table">
        <thead>
          <tr>
            <th scope="col">{{ t("viz.table.column.relation") }}</th>
            <th scope="col">{{ t("viz.table.column.from") }}</th>
            <th scope="col">{{ t("viz.table.column.to") }}</th>
            <th scope="col">{{ t("viz.table.column.nature") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="row in edgeRows"
            :key="row.edge.id"
            class="viz-table__row"
            :data-testid="'viz-edge-row'"
          >
            <td class="viz-table__cell">{{ row.edge.relation }}</td>
            <td class="viz-table__cell">{{ row.edge.from }}</td>
            <td class="viz-table__cell">{{ row.edge.to }}</td>
            <td class="viz-table__cell">
              <Badge v-if="row.derived" variant="warning" size="sm" outline>
                {{ t("viz.table.derived") }}
              </Badge>
              <span v-else class="viz-table__muted">
                {{ t("viz.table.direct") }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
    </section>
  </div>
</template>

<style scoped>
.viz-table {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.viz-table__block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.viz-table__heading {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.viz-table__table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.viz-table__table th {
  text-align: left;
  font-weight: var(--weight-medium);
  color: var(--text-tertiary);
  padding: var(--space-1) var(--space-2);
  border-bottom: var(--border-1) solid var(--border-default);
}

.viz-table__table td {
  padding: var(--space-1) var(--space-2);
  border-bottom: var(--border-1) solid var(--border-subtle);
}

.viz-table__row {
  cursor: pointer;
}

.viz-table__row:hover,
.viz-table__row:focus-visible {
  background: var(--surface-raised);
  outline: none;
}

.viz-table__cell--label {
  color: var(--text-primary);
}

.viz-table__muted {
  color: var(--text-tertiary);
}
</style>
