<script setup lang="ts">
import type { PcProposalDetail } from "@/api/knowledge";

/**
 * Proposal 业务上下文（V1.0 §5.2）：提案正文 / 变更说明 / 风险 / 版本。
 *
 * 纯展示件：数据来自 P4 详查（store 预载），文案由调用方翻译后经 props
 * 传入（D8）。只展示接口实际返回的字段——不伪造 diff（D9/V1.0 §5.2）。
 * stale 提示（R07：CAS 失配后必须重看最新内容）由调用方传入。
 */
interface Props {
  proposal: PcProposalDetail | null;
  loading: boolean;
  failed: boolean;
  stale: boolean;
  contextTitle: string;
  loadingLabel: string;
  failedLabel: string;
  rationaleLabel: string;
  riskLabel: string;
  confidenceLabel: string;
  governanceLabel: string;
  memberCountLabel: string;
  versionLabel: string;
  staleNotice: string;
}

defineProps<Props>();
</script>

<template>
  <section class="proposal-ctx" aria-label="context">
    <h3 class="proposal-ctx__title">{{ contextTitle }}</h3>

    <p v-if="loading" class="proposal-ctx__note">{{ loadingLabel }}</p>
    <p v-else-if="failed || !proposal" class="proposal-ctx__note">
      {{ failedLabel }}
    </p>

    <template v-else>
      <div
        v-if="stale"
        class="proposal-ctx__stale"
        role="alert"
        data-testid="proposal-stale"
      >
        {{ staleNotice }}
      </div>

      <p v-if="proposal.rationale" class="proposal-ctx__rationale">
        {{ proposal.rationale }}
      </p>

      <dl class="proposal-ctx__facts">
        <div class="proposal-ctx__fact">
          <dt>{{ riskLabel }}</dt>
          <dd>{{ proposal.risk_level }}</dd>
        </div>
        <div class="proposal-ctx__fact">
          <dt>{{ confidenceLabel }}</dt>
          <dd>{{ proposal.confidence }}</dd>
        </div>
        <div class="proposal-ctx__fact">
          <dt>{{ governanceLabel }}</dt>
          <dd>{{ proposal.governance_level }}</dd>
        </div>
        <div class="proposal-ctx__fact">
          <dt>{{ memberCountLabel }}</dt>
          <dd>{{ proposal.members.length }}</dd>
        </div>
        <div class="proposal-ctx__fact">
          <dt>{{ versionLabel }}</dt>
          <dd>v{{ proposal.expected_version_no }}</dd>
        </div>
      </dl>
    </template>
  </section>
</template>

<style scoped>
.proposal-ctx__title {
  margin: 0 0 8px;
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 600;
}

.proposal-ctx__stale {
  margin-bottom: 12px;
  padding: 10px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  color: var(--text-secondary);
  font-size: 13px;
  line-height: 1.5;
}

.proposal-ctx__rationale {
  margin: 0 0 12px;
  color: var(--text-primary);
  font-size: 14px;
  line-height: 1.6;
  white-space: pre-wrap;
}

.proposal-ctx__facts {
  display: flex;
  flex-wrap: wrap;
  gap: 16px 24px;
  margin: 0;
}

.proposal-ctx__fact dt {
  color: var(--text-secondary);
  font-size: 12px;
}

.proposal-ctx__fact dd {
  margin: 2px 0 0;
  color: var(--text-primary);
  font-size: 13px;
}

.proposal-ctx__note {
  margin: 0;
  color: var(--text-secondary);
  font-size: 13px;
}
</style>
