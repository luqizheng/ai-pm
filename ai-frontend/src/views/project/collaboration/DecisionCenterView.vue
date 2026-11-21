<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  cancelConsultation,
  decideConsultation,
  fetchDecisionCenter,
  type DecisionQueueItem
} from "@/api/collaboration";
import { decideApproval } from "@/api/pm";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 C — Human decision center（Figma 259:272，Decisions 页签）。
 *
 * 左 300px 决策队列（选中项 accent 底）+ 右详情（WHY HUMAN INVOLVEMENT /
 * AI-PM recommendation / OPTIONS & IMPACT / 动作）。两类队列行：
 *   - consultation → decide / cancel（理由必填）；
 *   - approval（红线闸 AI 提案）→ approved / rejected（理由必填）。
 * 底注（画布）：已决记录保留在项目决策登记；任何提案不自动生效。
 * 三反馈态：loading 骨架 / 空态（常规 AI 工作无需批准继续）/ error 不伪成功。
 */
const { t } = useI18n();
const router = useRouter();
const toast = useToast();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);
const items = ref<DecisionQueueItem[]>([]);
const waitingDecisionCount = ref(0);
const selectedId = ref<string | null>(null);

const decisionText = ref("");
const cancelText = ref("");
const acting = ref(false);

const pendingItems = computed(() =>
  items.value.filter(item => item.status === "pending")
);
const selectedItem = computed(
  () => items.value.find(item => item.id === selectedId.value) ?? null
);
const isConsultation = computed(
  () => selectedItem.value?.kind === "consultation"
);
const decided = computed(
  () => (selectedItem.value?.status ?? "pending") !== "pending"
);

function kindLabelClass(item: DecisionQueueItem): string {
  return item.kind === "approval" ? "dq-kind--danger" : "dq-kind--accent";
}

function optionsText(item: DecisionQueueItem): string {
  return item.options.map(option => option.label).join("\n");
}

function affectedWork(item: DecisionQueueItem): string {
  return item.taskId ? `T-${item.taskId.slice(0, 8)}` : "—";
}

function historyLine(item: DecisionQueueItem): string {
  const raised = new Date(item.createdAt).toLocaleString();
  if (item.status === "decided") {
    return `${raised} · ${item.decisionContent ?? ""}`;
  }
  if (item.status === "cancelled") {
    return `${raised} · ${item.cancelledReason ?? ""}`;
  }
  return `${raised} · ${t("pa06.decisions.historyPending")}`;
}

function select(item: DecisionQueueItem): void {
  selectedId.value = item.id;
  decisionText.value = "";
  cancelText.value = "";
}

async function load(): Promise<void> {
  if (!hasProject.value) {
    items.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const center = await fetchDecisionCenter(projectId.value);
    items.value = center.items;
    waitingDecisionCount.value = center.waitingDecisionCount;
    if (
      !selectedId.value ||
      !center.items.some(i => i.id === selectedId.value)
    ) {
      selectedId.value = center.items[0]?.id ?? null;
    }
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

/** consultation 决策：内容即决策记录（决定人=当前调用者，服务端执法） */
async function submitDecision(): Promise<void> {
  const item = selectedItem.value;
  if (!item || acting.value) return;
  if (isConsultation.value) {
    if (!decisionText.value.trim()) return;
    acting.value = true;
    try {
      await decideConsultation(item.id, decisionText.value.trim());
      toast.toast({ title: t("pa06.decisions.decidedToast") });
      await load();
    } catch (err) {
      toast.toast({ title: translateError(String(err)), variant: "danger" });
    } finally {
      acting.value = false;
    }
  }
}

/** 红线提案裁决：approved=执行提案；rejected=否决。理由必填（后端 400 执法） */
async function submitApproval(approved: boolean): Promise<void> {
  const item = selectedItem.value;
  if (!item || acting.value || !decisionText.value.trim()) return;
  acting.value = true;
  try {
    await decideApproval(item.id, {
      decision: approved ? "approved" : "rejected",
      reason: decisionText.value.trim()
    });
    toast.toast({
      title: approved
        ? t("pa06.decisions.approvedToast")
        : t("pa06.decisions.rejectedToast")
    });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

/** consultation 撤销（必填理由）——不是否决，是撤回这条咨询 */
async function submitCancel(): Promise<void> {
  const item = selectedItem.value;
  if (!item || acting.value || !cancelText.value.trim()) return;
  acting.value = true;
  try {
    await cancelConsultation(item.id, cancelText.value.trim());
    toast.toast({ title: t("pa06.decisions.cancelledToast") });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

function openTask(): void {
  const item = selectedItem.value;
  if (item?.taskId) {
    void router.push(`/project/tasks/${item.taskId}`);
  }
}

onMounted(load);
watch(projectId, load);
</script>

<template>
  <CollaborationFrame>
    <ScreenFeedback
      v-if="loading"
      state="loading"
      :title="$t('pa06.decisions.loadingEyebrow')"
    />
    <ScreenFeedback
      v-else-if="error"
      state="error"
      :description="error"
      @retry="load"
    />
    <template v-else>
      <CollabLayout>
        <template v-if="!hasProject">
          <ScreenFeedback state="empty" :title="$t('pa06.frame.noProject')" />
        </template>
        <!-- 全空态：无任何队列行（画布 Empty 态；空态不抹历史） -->
        <ScreenFeedback
          v-if="items.length === 0"
          state="empty"
          :title="$t('pa06.decisions.emptyEyebrow')"
          :description="$t('pa06.decisions.emptyHint')"
        >
          <p class="dc-register">{{ $t("pa06.decisions.registerNote") }}</p>
        </ScreenFeedback>
        <template v-else>
          <p class="dc-eyebrow">
            <template v-if="pendingItems.length > 0">
              {{
                $t("pa06.decisions.queueEyebrow", {
                  count: pendingItems.length
                })
              }}
            </template>
            <template v-else>
              {{ $t("pa06.decisions.decidedEyebrow") }}
            </template>
          </p>
          <div class="dc-body">
            <!-- 左：队列 -->
            <aside class="dc-queue" data-testid="dc-queue">
              <h2 class="dc-queue__title">
                {{ $t("pa06.decisions.queueTitle") }}
              </h2>
              <button
                v-for="item in items"
                :key="item.id"
                type="button"
                class="dc-queue__item"
                :class="{ 'is-selected': item.id === selectedId }"
                :data-testid="`dc-queue-item-${item.kind}`"
                @click="select(item)"
              >
                <span class="dc-queue__kind" :class="kindLabelClass(item)">
                  {{ item.kindLabel }}
                </span>
                <span class="dc-queue__title">{{ item.title }}</span>
                <span class="dc-queue__meta">{{ item.meta }}</span>
                <Badge
                  v-if="item.status !== 'pending'"
                  variant="neutral"
                  class="dc-queue__state"
                >
                  {{ item.status }}
                </Badge>
              </button>
              <p class="dc-queue__note">
                {{ $t("pa06.decisions.queueNote") }}
              </p>
            </aside>

            <!-- 右：详情 -->
            <section
              v-if="selectedItem"
              class="dc-detail"
              data-testid="dc-detail"
            >
              <Badge
                v-if="selectedItem.kind === 'approval'"
                variant="accent"
                data-testid="dc-proposal-badge"
              >
                {{ $t("pa06.decisions.proposalBadge") }}
              </Badge>
              <h2 class="dc-detail__title">{{ selectedItem.title }}</h2>
              <CollabContextSection
                :label="$t('pa06.decisions.whyLabel')"
                class="dc-detail__section"
              >
                {{ selectedItem.why }}
              </CollabContextSection>
              <CollabContextSection
                :label="$t('pa06.decisions.recommendationLabel')"
                class="dc-detail__section"
              >
                {{ selectedItem.recommendation }}
              </CollabContextSection>
              <CollabContextSection
                v-if="selectedItem.options.length > 0"
                :label="$t('pa06.decisions.optionsLabel')"
                class="dc-detail__section"
              >
                <pre class="dc-options">{{ optionsText(selectedItem) }}</pre>
              </CollabContextSection>

              <!-- 已决呈现（空态不抹历史） -->
              <div v-if="decided" class="dc-decided" data-testid="dc-decided">
                <p class="dc-decided__label">
                  {{ $t("pa06.decisions.decidedLabel") }}
                </p>
                <p class="dc-decided__content">
                  {{
                    selectedItem.decisionContent ?? selectedItem.cancelledReason
                  }}
                </p>
              </div>

              <!-- 动作区（pending 才可操作；刷新≠批准） -->
              <template v-if="!decided">
                <label class="dc-input-label" for="dc-decision-input">
                  {{ $t("pa06.decisions.decisionInputLabel") }}
                </label>
                <textarea
                  id="dc-decision-input"
                  v-model="decisionText"
                  class="dc-input"
                  rows="3"
                  :placeholder="$t('pa06.decisions.decisionInputPlaceholder')"
                  data-testid="dc-decision-input"
                />
                <p v-if="selectedItem.kind === 'approval'" class="dc-confirm">
                  {{ $t("pa06.decisions.confirmHint") }}
                </p>
                <div class="dc-actions">
                  <template v-if="selectedItem.kind === 'consultation'">
                    <Button
                      variant="primary"
                      size="sm"
                      :disabled="acting || decisionText.trim().length === 0"
                      data-testid="dc-decide"
                      @click="submitDecision"
                    >
                      {{ $t("pa06.decisions.decide") }}
                    </Button>
                  </template>
                  <template v-else>
                    <Button
                      variant="primary"
                      size="sm"
                      :disabled="acting || decisionText.trim().length === 0"
                      data-testid="dc-approve"
                      @click="submitApproval(true)"
                    >
                      {{ $t("pa06.decisions.approve") }}
                    </Button>
                    <Button
                      variant="secondary"
                      size="sm"
                      :disabled="acting || decisionText.trim().length === 0"
                      data-testid="dc-reject"
                      @click="submitApproval(false)"
                    >
                      {{ $t("pa06.decisions.reject") }}
                    </Button>
                  </template>
                </div>
                <template v-if="selectedItem.kind === 'consultation'">
                  <label class="dc-input-label" for="dc-cancel-input">
                    {{ $t("pa06.decisions.cancelInputLabel") }}
                  </label>
                  <textarea
                    id="dc-cancel-input"
                    v-model="cancelText"
                    class="dc-input"
                    rows="2"
                    :placeholder="$t('pa06.decisions.cancelInputPlaceholder')"
                    data-testid="dc-cancel-input"
                  />
                  <div class="dc-actions">
                    <Button
                      variant="secondary"
                      size="sm"
                      :disabled="acting || cancelText.trim().length === 0"
                      data-testid="dc-cancel"
                      @click="submitCancel"
                    >
                      {{ $t("pa06.decisions.cancelAction") }}
                    </Button>
                  </div>
                </template>
              </template>
            </section>
          </div>
          <p class="dc-register" data-testid="dc-register">
            {{ $t("pa06.decisions.registerNote") }}
          </p>
        </template>

        <!-- 右栏：Decision context -->
        <template #context>
          <CollabContextPanel :title="$t('pa06.decisions.ctxTitle')">
            <CollabContextSection :label="$t('pa06.decisions.ctxOwner')">
              {{ $t("pa06.decisions.ctxOwnerValue") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.decisions.ctxEvidence')">
              {{ selectedItem?.kindLabel ?? "—" }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.decisions.ctxAffected')">
              {{ selectedItem ? affectedWork(selectedItem) : "—" }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.decisions.ctxContinues')">
              {{ $t("pa06.decisions.ctxContinuesText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.decisions.ctxHistory')">
              {{
                selectedItem
                  ? historyLine(selectedItem)
                  : $t("pa06.decisions.historyPending")
              }}
            </CollabContextSection>
            <p class="ctx-meta">
              {{
                $t("pa06.decisions.ctxWaitingNote", {
                  count: waitingDecisionCount
                })
              }}
            </p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.dc-eyebrow {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.dc-body {
  display: flex;
  align-items: flex-start;
  gap: var(--space-4);
}

.dc-queue {
  flex: 0 0 300px;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-3);
}

.dc-queue__title {
  margin: 0 0 var(--space-1);
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.dc-queue__item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-1);
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  padding: var(--space-3);
  text-align: left;
  cursor: pointer;
}

.dc-queue__item.is-selected {
  background: color-mix(in srgb, var(--ws-accent) 12%, transparent);
}

.dc-queue__item:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
}

.dc-queue__kind {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.dc-queue__kind--accent {
  color: var(--ws-accent);
}

.dc-queue__kind--danger {
  color: var(--ws-color-danger-text);
}

.dc-queue__title {
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.dc-queue__meta {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.dc-queue__state {
  align-self: flex-start;
}

.dc-queue__note {
  margin: var(--space-2) 0 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-secondary);
}

.dc-detail {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.dc-detail__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: 1.375rem;
  font-weight: var(--weight-semibold);
  letter-spacing: -0.018em;
  color: var(--text-primary);
}

.dc-detail__section {
  width: 100%;
}

.dc-options {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border: var(--border-1) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
  white-space: pre-wrap;
}

.dc-decided {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  width: 100%;
}

.dc-decided__label {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.dc-decided__content {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.dc-input-label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  color: var(--text-secondary);
}

.dc-input {
  width: 100%;
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-2) var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
  resize: vertical;
}

.dc-input:focus {
  outline: none;
  border: var(--border-2) solid var(--ws-accent);
}

.dc-confirm {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.dc-actions {
  display: flex;
  gap: var(--space-2);
}

.dc-register {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.ctx-meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: 1.45;
  color: var(--text-secondary);
}
</style>
