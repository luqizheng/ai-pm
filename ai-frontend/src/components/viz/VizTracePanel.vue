<script setup lang="ts">
/**
 * VizTracePanel —— 溯源侧栏。
 *
 * 复用 `ui/sheet/Sheet.vue` 范式（业务同构参照：requirement-detail 的 SourceSheet）。
 * 展示所选节点的来源、关系与性质；**不自行查权限**——节点来自已裁剪的 spec，
 * 「打开原文档」走目标域自身路由，由该路由的权限门裁决。
 *
 * 计数只反映**裁剪后**的可见集合（服务端口径），不显示全量规模——
 * 否则计数差异会成为探测通道（14 号 §3.2 / D-2）。
 */
import { computed } from "vue";
import { ExternalLink, Link2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import type { VizEdge, VizNode, VizSpec } from "@/lib/viz/types";
import Badge from "@/components/ui/badge/Badge.vue";
import Sheet from "@/components/ui/sheet/Sheet.vue";

interface Props {
  open: boolean;
  spec: VizSpec;
  node: VizNode | null;
}

const props = defineProps<Props>();

const { t } = useI18n();

const emit = defineEmits<{
  "update:open": [value: boolean];
  openSource: [sourceRef: NonNullable<VizNode["source_ref"]>];
}>();

/** 入边 / 出边（按 spec 裁剪后的集合）。 */
const outgoing = computed<VizEdge[]>(() =>
  props.node
    ? props.spec.edges.filter(edge => edge.from === props.node?.id)
    : []
);
const incoming = computed<VizEdge[]>(() =>
  props.node ? props.spec.edges.filter(edge => edge.to === props.node?.id) : []
);

function close(): void {
  emit("update:open", false);
}
</script>

<template>
  <Sheet
    :open="open"
    :title="node?.label || t('viz.trace.untitled')"
    :description="t('viz.trace.description')"
    :close-label="t('viz.trace.close')"
    side="right"
    @update:open="emit('update:open', $event)"
  >
    <div v-if="node" class="viz-trace" :data-testid="'viz-trace-panel'">
      <section class="viz-trace__section">
        <h4 class="viz-trace__heading">{{ t("viz.trace.identity") }}</h4>
        <dl class="viz-trace__list">
          <div class="viz-trace__row">
            <dt>{{ t("viz.trace.kind") }}</dt>
            <dd>{{ node.kind }}</dd>
          </div>
          <div v-if="node.status" class="viz-trace__row">
            <dt>{{ t("viz.trace.status") }}</dt>
            <dd>{{ node.status }}</dd>
          </div>
          <div class="viz-trace__row">
            <dt>{{ t("viz.trace.nature") }}</dt>
            <dd>
              <Badge
                :variant="node.nature === 'explanatory' ? 'warning' : 'neutral'"
                size="sm"
                :outline="node.nature === 'explanatory'"
              >
                {{
                  node.nature === "explanatory"
                    ? t("viz.artifact.explanatory")
                    : t("viz.trace.business")
                }}
              </Badge>
            </dd>
          </div>
        </dl>
      </section>

      <section class="viz-trace__section">
        <h4 class="viz-trace__heading">{{ t("viz.trace.source") }}</h4>
        <template v-if="node.source_ref">
          <dl class="viz-trace__list">
            <div class="viz-trace__row">
              <dt>{{ t("viz.trace.domain") }}</dt>
              <dd>{{ node.source_ref.domain }}</dd>
            </div>
            <div class="viz-trace__row">
              <dt>{{ t("viz.trace.type") }}</dt>
              <dd>{{ node.source_ref.type }}</dd>
            </div>
            <div class="viz-trace__row">
              <dt>{{ t("viz.trace.id") }}</dt>
              <dd>{{ node.source_ref.id }}</dd>
            </div>
            <div v-if="node.source_ref.revision_id" class="viz-trace__row">
              <dt>{{ t("viz.trace.revision") }}</dt>
              <dd>{{ node.source_ref.revision_id }}</dd>
            </div>
          </dl>
          <button
            type="button"
            class="viz-trace__link"
            :data-testid="'viz-open-source'"
            @click="emit('openSource', node.source_ref)"
          >
            <ExternalLink :size="14" aria-hidden="true" />
            <span>{{ t("viz.trace.openSource") }}</span>
          </button>
        </template>
        <p v-else class="viz-trace__unlinked" :data-testid="'viz-unlinked'">
          {{ t("viz.trace.unlinkedHint") }}
        </p>
      </section>

      <section class="viz-trace__section">
        <h4 class="viz-trace__heading">
          {{
            t("viz.trace.relationships", {
              out: outgoing.length,
              in: incoming.length
            })
          }}
        </h4>
        <ul
          v-if="outgoing.length || incoming.length"
          class="viz-trace__relations"
        >
          <li
            v-for="edge in outgoing"
            :key="`out-${edge.id}`"
            class="viz-trace__relation"
          >
            <Link2 :size="12" aria-hidden="true" />
            <span class="viz-trace__relation-label">{{ edge.relation }}</span>
            <span class="viz-trace__relation-target">→ {{ edge.to }}</span>
            <Badge
              v-if="edge.derived && edge.derived.direct === false"
              variant="warning"
              size="sm"
              outline
            >
              {{
                t("viz.trace.derivedVia", { via: edge.derived.via.join(" → ") })
              }}
            </Badge>
          </li>
          <li
            v-for="edge in incoming"
            :key="`in-${edge.id}`"
            class="viz-trace__relation"
          >
            <Link2 :size="12" aria-hidden="true" />
            <span class="viz-trace__relation-label">{{ edge.relation }}</span>
            <span class="viz-trace__relation-target">← {{ edge.from }}</span>
            <Badge
              v-if="edge.derived && edge.derived.direct === false"
              variant="warning"
              size="sm"
              outline
            >
              {{
                t("viz.trace.derivedVia", { via: edge.derived.via.join(" → ") })
              }}
            </Badge>
          </li>
        </ul>
        <p v-else class="viz-trace__muted">{{ t("viz.trace.noRelations") }}</p>
      </section>

      <button type="button" class="viz-trace__close" @click="close">
        {{ t("viz.trace.close") }}
      </button>
    </div>
  </Sheet>
</template>

<style scoped>
.viz-trace {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.viz-trace__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.viz-trace__heading {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.viz-trace__list {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.viz-trace__row {
  display: flex;
  justify-content: space-between;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
}

.viz-trace__row dt {
  color: var(--text-tertiary);
}

.viz-trace__row dd {
  margin: 0;
  color: var(--text-primary);
  text-align: right;
  word-break: break-all;
}

.viz-trace__link,
.viz-trace__close {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  align-self: flex-start;
  padding: 0 var(--space-2);
  height: var(--size-control-md);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--accent);
  background: transparent;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.viz-trace__relations {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.viz-trace__relation {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  flex-wrap: wrap;
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.viz-trace__relation-label {
  color: var(--text-primary);
}

.viz-trace__unlinked,
.viz-trace__muted {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
