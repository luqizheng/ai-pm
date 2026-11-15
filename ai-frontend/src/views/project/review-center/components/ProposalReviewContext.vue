<script setup lang="ts">
import Alert from "@/components/ui/alert/Alert.vue";
import Descriptions from "@/components/ui/descriptions/Descriptions.vue";
import type { PcProposalDetail } from "@/api/knowledge";

/**
 * Proposal 业务上下文（V1.0 §5.2）：提案正文 / 变更说明 / 风险 / 版本。
 *
 * stale 提示 = DS 组件 Alert（warning）；键值区 = DS 组件 Descriptions。
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
      <Alert
        v-if="stale"
        class="proposal-ctx__stale"
        variant="warning"
        size="sm"
        role="alert"
        data-testid="proposal-stale"
      >
        {{ staleNotice }}
      </Alert>

      <p v-if="proposal.rationale" class="proposal-ctx__rationale">
        {{ proposal.rationale }}
      </p>

      <Descriptions
        :items="[
          { label: riskLabel, value: proposal.risk_level },
          { label: confidenceLabel, value: proposal.confidence },
          { label: governanceLabel, value: proposal.governance_level },
          { label: memberCountLabel, value: proposal.members.length },
          {
            label: versionLabel,
            value: `v${proposal.expected_version_no}`
          }
        ]"
      />
    </template>
  </section>
</template>

<style scoped>
.proposal-ctx__title {
  margin: 0 0 var(--space-2);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
}

.proposal-ctx__stale {
  margin-bottom: var(--space-3);
}

.proposal-ctx__rationale {
  margin: 0 0 var(--space-3);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.6;
  white-space: pre-wrap;
}

.proposal-ctx__note {
  margin: 0;
  color: var(--text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}
</style>
