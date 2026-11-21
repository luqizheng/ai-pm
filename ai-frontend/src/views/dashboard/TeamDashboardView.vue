<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { HttpError } from "@digua/http";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from "@/components/ui/table";
import {
  listProductMembers,
  listProducts,
  type ProductProfile
} from "@/api/products";
import { listTeamMembers, type TeamMember } from "@/api/team";
import { useAuthStore } from "@/stores/auth";
import { useTeamStore } from "@/stores/team";
import TeamDomainTabs from "@/views/team/TeamDomainTabs.vue";
import TeamRestrictedState from "@/views/team/TeamRestrictedState.vue";

/**
 * Team 概览（P4-ORG UX 修订 V1.0 / Figma T1「Team overview」）。
 *
 * 团队档案卡（Owner 名单 / 成员与产品计数）+ 团队名下 Products 表
 * （Lifecycle / Product Owner / Access）。Access 判定与后端同源：能读该
 * Product 成员表（listProductMembers）即有资格「打开 Product」，404/403
 * = Restricted；abandoned Product 直接呈现受限文本。组织隶属不授予
 * Product Membership——信息横幅明示该边界（呈现态，授权在后端）。
 */
const { t } = useI18n();
const router = useRouter();
const auth = useAuthStore();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const restricted = ref(false);
const members = ref<TeamMember[]>([]);
const products = ref<ProductProfile[]>([]);

/** 每个产品一行：Product Owner 名与当前用户是否有资格打开 */
interface ProductRow {
  product: ProductProfile;
  ownerName: string | null;
  eligible: boolean;
}

const productRows = ref<ProductRow[]>([]);

const teamId = computed(() => teamStore.context?.team?.id ?? "");
const team = computed(() => teamStore.currentTeam);
const organization = computed(() => auth.organization ?? "");

/** 档案标题：有 slug 时「{slug} · {name}」（设计稿 PE · Platform Engineering） */
const summaryTitle = computed(() =>
  team.value?.slug
    ? `${team.value.slug} · ${team.value.name}`
    : (team.value?.name ?? "")
);

const activeOwners = computed(() =>
  members.value.filter(member => member.role === "owner" && member.is_active)
);

const ownersNames = computed(() =>
  activeOwners.value.map(member => member.display_name).join(" · ")
);

const statsLine = computed(() => {
  const people = members.value.length;
  const owners = activeOwners.value.length;
  const membersCount = members.value.filter(
    member => member.role !== "owner"
  ).length;
  return t("team.dashboard.statsLine", {
    people,
    owners,
    members: membersCount,
    products: products.value.length
  });
});

async function reload(): Promise<void> {
  if (!teamId.value) return;
  loading.value = true;
  error.value = null;
  restricted.value = false;
  try {
    const [memberRows, productRows_] = await Promise.all([
      listTeamMembers(teamId.value),
      listProducts(teamId.value)
    ]);
    members.value = memberRows;
    products.value = productRows_;
    // Access 与 Product Owner 逐产品核对（读成员表 = 资格判定；产品规模为
    // 小团队量级，N+1 可接受）。
    productRows.value = await Promise.all(
      productRows_.map(async product => {
        try {
          const rows = await listProductMembers(product.id);
          const owner = rows.find(row => row.role === "owner" && row.is_active);
          return {
            product,
            ownerName: owner?.display_name ?? null,
            eligible: true
          };
        } catch {
          // 非该 Product 成员（后端 404）：如实呈现受限，不冒充资格。
          return { product, ownerName: null, eligible: false };
        }
      })
    );
  } catch (err) {
    if (err instanceof HttpError && err.status === 403) {
      restricted.value = true;
    } else {
      error.value =
        err instanceof Error ? err.message : t("team.dashboard.loadFailed");
    }
  } finally {
    loading.value = false;
  }
}

function openProduct(): void {
  // Product 管理面 = 团队设置 Products 页（产品详情页随 M 系列另行落地）。
  void router.push("/settings/team/products");
}

onMounted(async () => {
  await teamStore.load();
  await reload();
});
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: $t('team.tabs.domain') },
      { label: team?.name ?? '' }
    ]"
    :title="team?.name ?? ''"
    :description="
      organization
        ? $t('team.dashboard.subtitle', { org: organization })
        : undefined
    "
  >
    <TeamDomainTabs />

    <TeamRestrictedState v-if="restricted" />

    <template v-else>
      <p v-if="error" class="td-error">{{ error }}</p>

      <!-- 团队档案卡（T1 Team summary） -->
      <section class="td-summary" data-testid="team-overview-summary">
        <div class="td-summary__head">
          <h2 class="td-summary__title">{{ summaryTitle }}</h2>
          <Badge variant="accent">{{ $t("team.dashboard.teamBadge") }}</Badge>
        </div>
        <p class="td-summary__desc">{{ team?.description || "—" }}</p>
        <div class="td-summary__meta">
          <span data-testid="team-overview-owners">
            {{ $t("team.dashboard.ownersLine", { names: ownersNames || "—" }) }}
          </span>
          <span data-testid="team-overview-stats">{{ statsLine }}</span>
        </div>
      </section>

      <!-- 团队名下 Products（T1 Products in this Team） -->
      <section class="td-card" data-testid="team-overview-products">
        <h2 class="td-card__title">{{ $t("team.dashboard.productsTitle") }}</h2>
        <p v-if="!loading && productRows.length === 0" class="td-card__empty">
          {{ $t("team.products.empty") }}
        </p>
        <Table v-else>
          <TableHeader>
            <TableRow>
              <TableHead>{{ $t("team.dashboard.colProduct") }}</TableHead>
              <TableHead>{{ $t("team.dashboard.colLifecycle") }}</TableHead>
              <TableHead>{{ $t("team.dashboard.colOwner") }}</TableHead>
              <TableHead>{{ $t("team.dashboard.colAccess") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="row in productRows"
              :key="row.product.id"
              :data-testid="`team-product-row-${row.product.id}`"
            >
              <TableCell class="td-product-name">
                {{ row.product.name }}
              </TableCell>
              <TableCell>
                <Badge variant="neutral">
                  {{
                    row.product.status === "abandoned"
                      ? $t("team.dashboard.lifecycleAbandoned")
                      : $t("team.dashboard.lifecycleActive")
                  }}
                </Badge>
              </TableCell>
              <TableCell>{{ row.ownerName ?? "—" }}</TableCell>
              <TableCell>
                <Button
                  v-if="row.eligible"
                  variant="secondary"
                  size="sm"
                  :data-test="`team-product-open-${row.product.id}`"
                  @click="openProduct"
                >
                  {{ $t("team.dashboard.openProduct") }}
                </Button>
                <Button
                  v-else-if="row.product.status !== 'abandoned'"
                  variant="secondary"
                  size="sm"
                  disabled
                  :title="$t('team.dashboard.restrictedHint')"
                >
                  {{ $t("team.dashboard.restricted") }}
                </Button>
                <span v-else>{{ $t("team.dashboard.restricted") }}</span>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </section>

      <!-- 边界说明（T1 Info 横幅） -->
      <Alert variant="info" :title="$t('team.dashboard.affiliationTitle')">
        {{ $t("team.dashboard.affiliationDetail") }}
      </Alert>
    </template>
  </PageFrame>
</template>

<style scoped>
.td-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.td-summary {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-5);
}

.td-summary__head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.td-summary__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.td-summary__desc {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.td-summary__meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-5);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.td-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.td-card__title {
  margin: 0 0 var(--space-4);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.td-card__empty {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.td-product-name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}
</style>
