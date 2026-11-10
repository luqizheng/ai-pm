<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ArrowLeft, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import {
  getCollaborationGraph,
  type CollaborationGraph
} from "@/api/execution-plans";
import ExecutionFrame from "./ExecutionFrame.vue";
import { runLabel, runTone, taskTone } from "./execution-contract";

/**
 * 多 Agent 协作（S05-03，接真）。
 *
 * 数据 = GET /tasks/{taskId}/collaboration：子任务节点（含最新 Run 与
 * Agent）+ 依赖边 + Agent 名册。
 *
 * **未完成上游数**是阻塞判定的直接读数——前端不重算拓扑。
 * Agent 用角色名 + 设备名表达，无装饰性头像。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const taskId = computed(() => String(route.params.taskId ?? ""));
const graph = ref<CollaborationGraph | null>(null);
const loading = ref(true);
const loadError = ref(false);

const kindLabels = computed(() => ({
  delivery: t("s05.execution.runKind.delivery"),
  review: t("s05.execution.runKind.review")
}));

async function load() {
  if (!taskId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    graph.value = await getCollaborationGraph(taskId.value);
  } catch {
    loadError.value = true;
    graph.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(taskId, load);

const nodes = computed(() => graph.value?.nodes ?? []);
const edges = computed(() => graph.value?.edges ?? []);
const agents = computed(() => graph.value?.agents ?? []);
const blockedCount = computed(
  () => nodes.value.filter(node => node.open_upstream_count > 0).length
);

function goBack() {
  void router.push("/project/execution");
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s05.execution.module') },
      { label: t('s05.execution.page.centerTitle'), to: '/project/execution' }
    ]"
    :title="t('s05.execution.page.collaborationTitle')"
    :description="t('s05.execution.page.collaborationDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="co-back"
        @click="goBack"
      >
        <ArrowLeft :size="14" />
        {{ t("s05.execution.common.back") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="co-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s05.execution.common.refresh") }}
      </Button>
    </template>

    <ExecutionFrame>
      <Alert
        v-if="loadError"
        variant="danger"
        size="sm"
        :title="t('s05.execution.common.errorTitle')"
      />

      <EmptyState
        v-else-if="!loading && !graph"
        :title="t('s05.execution.collaboration.notFoundTitle')"
        :description="t('s05.execution.collaboration.notFoundHint')"
      />

      <template v-else-if="graph">
        <Card class="co-section" data-testid="co-nodes">
          <template #header>
            {{
              t("s05.execution.collaboration.nodesTitle", {
                count: nodes.length
              })
            }}
          </template>
          <p v-if="nodes.length === 0" class="co-empty">
            {{ t("s05.execution.collaboration.nodesEmpty") }}
          </p>
          <ul v-else class="co-list">
            <li
              v-for="node in nodes"
              :key="node.id"
              class="co-row"
              :data-testid="`co-node-${node.id.slice(0, 8)}`"
            >
              <span class="co-row__title">{{ node.title }}</span>
              <Badge :variant="taskTone(node.status)" size="sm">
                {{ t(`s05.execution.taskStatus.${node.status}`, node.status) }}
              </Badge>
              <Badge
                v-if="node.run_status"
                :variant="runTone(node.run_status)"
                size="sm"
              >
                {{
                  t(
                    `s05.execution.runStatus.${node.run_status}`,
                    node.run_status
                  )
                }}
              </Badge>
              <span v-if="node.run_id" class="co-meta">
                {{ runLabel(node.run_kind, node.attempt_no, kindLabels) }}
              </span>
              <span class="co-meta">{{
                node.agent_name ?? t("s05.execution.common.noAgent")
              }}</span>
              <Badge
                v-if="node.open_upstream_count > 0"
                variant="warning"
                size="sm"
              >
                {{
                  t("s05.execution.collaboration.blockedBy", {
                    n: node.open_upstream_count
                  })
                }}
              </Badge>
            </li>
          </ul>
        </Card>

        <Card class="co-section" data-testid="co-edges">
          <template #header>
            {{
              t("s05.execution.collaboration.edgesTitle", {
                count: edges.length
              })
            }}
          </template>
          <p v-if="edges.length === 0" class="co-empty">
            {{ t("s05.execution.collaboration.edgesEmpty") }}
          </p>
          <ul v-else class="co-list">
            <li
              v-for="edge in edges"
              :key="`${edge.from_task_id}-${edge.to_task_id}`"
              class="co-row"
            >
              <span class="co-mono">{{ edge.from_task_id.slice(0, 8) }}</span>
              <span class="co-meta">→</span>
              <span class="co-mono">{{ edge.to_task_id.slice(0, 8) }}</span>
            </li>
          </ul>
        </Card>

        <Card class="co-section" data-testid="co-agents">
          <template #header>
            {{
              t("s05.execution.collaboration.agentsTitle", {
                count: agents.length
              })
            }}
          </template>
          <p v-if="agents.length === 0" class="co-empty">
            {{ t("s05.execution.collaboration.agentsEmpty") }}
          </p>
          <ul v-else class="co-list">
            <li
              v-for="agent in agents"
              :key="agent.agent_id"
              class="co-row"
              :data-testid="`co-agent-${agent.agent_id.slice(0, 8)}`"
            >
              <span class="co-row__title">
                {{ agent.agent_name ?? t("s05.execution.common.noAgent") }}
              </span>
              <span class="co-meta">
                {{
                  t("s05.execution.collaboration.agentRuns", {
                    n: agent.run_count
                  })
                }}
              </span>
              <Badge v-if="agent.active_run_count > 0" variant="info" size="sm">
                {{
                  t("s05.execution.collaboration.agentActive", {
                    n: agent.active_run_count
                  })
                }}
              </Badge>
            </li>
          </ul>
        </Card>

        <p v-if="blockedCount > 0" class="co-empty" data-testid="co-blocked">
          {{
            t("s05.execution.collaboration.blockedSummary", { n: blockedCount })
          }}
        </p>
      </template>
    </ExecutionFrame>
  </PageFrame>
</template>

<style scoped>
.co-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.co-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.co-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.co-row__title {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

.co-meta {
  color: var(--ws-text-secondary);
  font-variant-numeric: tabular-nums;
}

.co-mono {
  font-family: var(--font-mono);
  color: var(--ws-text-primary);
}

.co-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
