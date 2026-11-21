<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import {
  confirmProposal,
  listDocuments,
  listProposals,
  vetoRevision,
  type DocumentProposal,
  type KnowledgeDocumentSummary
} from "@/api/documents";
import { getDocument } from "@/api/documents";
import { fetchTeamContext } from "@/api/team";
import { useDesignProject } from "@/views/project/design/use-design-project";

import CollaborationFrame from "./CollaborationFrame.vue";
import CollabLayout from "./components/CollabLayout.vue";
import CollabContextPanel from "./components/CollabContextPanel.vue";
import CollabContextSection from "./components/CollabContextSection.vue";
import ScreenFeedback from "./components/ScreenFeedback.vue";

/**
 * PA-06 屏 D — Knowledge change review（Figma 259:379，Knowledge 页签）。
 *
 * 「审语义，不止审措辞」：待决知识提案队列 + 已确认基线 vs AI 提案双列
 * 对照 + WHY THIS CHANGE + 动作（Confirm 基线 / Veto 否决）。数据源=
 * documents 提案链（PA-06-03：AI 自主维护草稿，正式变更人类确认——
 * 质量门/红线提案→confirm，已落修订→veto）。
 *
 * 诚实边界：提案行不携带 markdown 全文——提案列呈现 change_summary +
 * 状态与目标修订；「精确 diff 请打开文档」（不伪造全文对照）。
 */
const { t } = useI18n();
const toast = useToast();
const { projectId, hasProject } = useDesignProject();

const loading = ref(true);
const error = ref<string | null>(null);
const proposals = ref<DocumentProposal[]>([]);
const documents = ref<KnowledgeDocumentSummary[]>([]);
const selectedId = ref<string>("");
const baselineExcerpt = ref<string | null>(null);
const acting = ref(false);

const PENDING_STATES = new Set(["pending", "redline_pending", "submitted"]);

const pendingProposals = computed(() =>
  proposals.value.filter(row => PENDING_STATES.has(row.status))
);

const selected = computed(
  () => proposals.value.find(row => row.id === selectedId.value) ?? null
);

const baselineDoc = computed(() => {
  if (!selected.value) return null;
  return (
    documents.value.find(doc => doc.doc_key === selected.value?.doc_key) ?? null
  );
});

function excerpt(markdown: string | null): string {
  if (!markdown) return "";
  const lines = markdown.split("\n").filter(line => line.trim().length > 0);
  return lines.slice(0, 4).join("\n");
}

async function loadBaseline(markdownWanted: boolean): Promise<void> {
  baselineExcerpt.value = null;
  const doc = baselineDoc.value;
  if (!doc || !markdownWanted) return;
  try {
    const full = await getDocument(doc.id, projectId.value);
    baselineExcerpt.value = excerpt(full.markdown);
  } catch {
    baselineExcerpt.value = null;
  }
}

async function load(): Promise<void> {
  if (!hasProject.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const id = projectId.value;
    const [rows, docs] = await Promise.all([
      listProposals(id),
      listDocuments(id).catch(() => ({
        items: [] as KnowledgeDocumentSummary[]
      }))
    ]);
    proposals.value = rows.items ?? [];
    documents.value = docs.items ?? [];
    if (
      !selectedId.value ||
      !proposals.value.some(p => p.id === selectedId.value)
    ) {
      selectedId.value = pendingProposals.value[0]?.id ?? "";
    }
    await loadBaseline(true);
  } catch (err) {
    error.value = translateError(String(err));
  } finally {
    loading.value = false;
  }
}

function select(row: DocumentProposal): void {
  selectedId.value = row.id;
  void loadBaseline(true);
}

/** 确认红线提案：confirmed_by=当前调用者；operation_id 幂等键 */
async function confirmBaseline(): Promise<void> {
  const row = selected.value;
  if (!row || acting.value) return;
  acting.value = true;
  try {
    const me = await fetchTeamContext();
    await confirmProposal(
      row.id,
      projectId.value,
      me.principal_id,
      crypto.randomUUID()
    );
    toast.toast({ title: t("pa06.knowledge.confirmedToast") });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
  }
}

/** 否决已落修订（resulting_revision_id 存在时可用） */
async function vetoProposal(): Promise<void> {
  const row = selected.value;
  const revisionId = row?.resulting_revision_id;
  if (!row || !revisionId || acting.value) return;
  acting.value = true;
  try {
    const me = await fetchTeamContext();
    await vetoRevision(
      revisionId,
      projectId.value,
      me.principal_id,
      t("pa06.knowledge.vetoReason"),
      crypto.randomUUID()
    );
    toast.toast({ title: t("pa06.knowledge.vetoedToast") });
    await load();
  } catch (err) {
    toast.toast({ title: translateError(String(err)), variant: "danger" });
  } finally {
    acting.value = false;
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
      :title="$t('pa06.knowledge.loadingEyebrow')"
    />
    <ScreenFeedback
      v-else-if="error"
      state="error"
      :description="error"
      @retry="load"
    />
    <template v-else>
      <template v-if="!hasProject">
        <ScreenFeedback state="empty" :title="$t('pa06.frame.noProject')" />
      </template>
      <!-- 空态：无待决知识提案（常规草稿自主维护无需批准） -->
      <ScreenFeedback
        v-else-if="pendingProposals.length === 0"
        state="empty"
        :title="$t('pa06.knowledge.emptyEyebrow')"
        :description="$t('pa06.knowledge.emptyHint')"
      />
      <CollabLayout v-else>
        <div class="kb-head">
          <div class="kb-head__eyebrow-row">
            <span class="kb-eyebrow">
              {{ selected?.doc_key }} · {{ selected?.doc_type }}
            </span>
            <Badge variant="accent">
              {{ $t("pa06.knowledge.confirmBadge") }}
            </Badge>
          </div>
          <h2 class="kb-head__title">
            {{ $t("pa06.knowledge.headline") }}
          </h2>
          <p class="kb-head__meta">
            {{
              $t("pa06.knowledge.headlineMeta", {
                summary: selected?.change_summary ?? "—"
              })
            }}
          </p>
          <!-- 队列（多提案时切换） -->
          <div v-if="pendingProposals.length > 1" class="kb-queue">
            <button
              v-for="row in pendingProposals"
              :key="row.id"
              type="button"
              class="kb-queue__item"
              :class="{ 'is-selected': row.id === selectedId }"
              :data-testid="`kb-queue-${row.doc_key}`"
              @click="select(row)"
            >
              {{ row.doc_key }} · {{ row.change_summary || row.status }}
            </button>
          </div>
        </div>

        <!-- 双列对照：已确认基线 vs AI 提案 -->
        <div class="kb-diff">
          <div class="kb-col" data-testid="kb-baseline">
            <span class="kb-eyebrow">{{
              $t(
                "pa06.knowledge.baselineEyebrow",
                baselineDoc ? { rev: baselineDoc.revision_no } : { rev: "—" }
              )
            }}</span>
            <p class="kb-col__title">
              {{ baselineExcerpt || $t("pa06.knowledge.baselineFallback") }}
            </p>
            <p class="kb-col__meta">
              {{
                baselineDoc
                  ? $t("pa06.knowledge.baselineMeta", {
                      rev: baselineDoc.revision_no,
                      time: new Date(baselineDoc.updated_at).toLocaleString()
                    })
                  : $t("pa06.knowledge.baselineMissing")
              }}
            </p>
          </div>
          <div class="kb-col kb-col--proposal" data-testid="kb-proposal">
            <span class="kb-eyebrow">{{
              $t("pa06.knowledge.proposalEyebrow")
            }}</span>
            <p class="kb-col__title kb-col__title--accent">
              {{ selected?.change_summary || selected?.doc_key }}
            </p>
            <p class="kb-col__meta">
              {{
                $t("pa06.knowledge.proposalMeta", {
                  status: selected?.status ?? ""
                })
              }}
            </p>
          </div>
        </div>

        <!-- WHY THIS CHANGE -->
        <CollabContextSection
          :label="$t('pa06.knowledge.whyLabel')"
          class="kb-why"
        >
          {{ selected?.change_summary || $t("pa06.knowledge.whyFallback") }}
        </CollabContextSection>

        <!-- 变更影响 -->
        <div class="kb-card" data-testid="kb-impact">
          <p class="kb-impact__line">
            {{
              $t("pa06.knowledge.impactRecords", {
                key: selected?.doc_key ?? ""
              })
            }}
          </p>
          <p class="kb-impact__meta">
            {{
              $t("pa06.knowledge.impactRevision", {
                rev: selected?.resulting_revision_id?.slice(0, 8) ?? "—"
              })
            }}
          </p>
        </div>

        <!-- 动作：确认基线 / 否决 -->
        <div class="kb-actions">
          <Button
            variant="primary"
            size="sm"
            :disabled="acting"
            data-testid="kb-confirm"
            @click="confirmBaseline"
          >
            {{ $t("pa06.knowledge.confirmAction") }}
          </Button>
          <Button
            v-if="selected?.resulting_revision_id"
            variant="secondary"
            size="sm"
            :disabled="acting"
            data-testid="kb-veto"
            @click="vetoProposal"
          >
            {{ $t("pa06.knowledge.vetoAction") }}
          </Button>
        </div>
        <p class="kb-note">{{ $t("pa06.knowledge.actionNote") }}</p>

        <template #context>
          <CollabContextPanel :title="$t('pa06.knowledge.ctxTitle')">
            <p class="kb-ctx-text">
              {{ $t("pa06.knowledge.ctxIntro") }}
            </p>
            <CollabContextSection :label="$t('pa06.knowledge.ctxNext')">
              {{ $t("pa06.knowledge.ctxNextText") }}
            </CollabContextSection>
            <CollabContextSection :label="$t('pa06.knowledge.ctxWorking')">
              {{ $t("pa06.knowledge.ctxWorkingText") }}
            </CollabContextSection>
            <p class="kb-ctx-note">{{ $t("pa06.knowledge.ctxNote") }}</p>
          </CollabContextPanel>
        </template>
      </CollabLayout>
    </template>
  </CollaborationFrame>
</template>

<style scoped>
.kb-eyebrow {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.kb-head {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
}

.kb-head__eyebrow-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: var(--space-3);
}

.kb-head__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: 1.375rem;
  font-weight: var(--weight-semibold);
  letter-spacing: -0.018em;
  color: var(--text-primary);
}

.kb-head__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.kb-queue {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.kb-queue__item {
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-1) var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  cursor: pointer;
}

.kb-queue__item.is-selected {
  background: color-mix(in srgb, var(--ws-accent) 12%, transparent);
  border-color: var(--ws-accent);
  color: var(--text-primary);
}

.kb-diff {
  display: flex;
  gap: var(--space-4);
}

.kb-col {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-4);
}

.kb-col--proposal {
  background: color-mix(in srgb, var(--ws-accent) 8%, transparent);
  border-color: var(--ws-accent);
}

.kb-col__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  white-space: pre-line;
}

.kb-col__title--accent {
  color: var(--ws-accent);
}

.kb-col__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.kb-why {
  width: 100%;
}

.kb-card {
  border: var(--border-1) solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  padding: var(--space-3) var(--space-4);
}

.kb-impact__line {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.kb-impact__meta {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.kb-actions {
  display: flex;
  gap: var(--space-2);
}

.kb-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.kb-ctx-text {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.kb-ctx-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

@media (max-width: 900px) {
  .kb-diff {
    flex-direction: column;
  }
}
</style>
