<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { HttpError } from "@digua/http";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from "@/components/ui/table";
import {
  listProductLinks,
  listProductMembers,
  listProducts,
  type ProductProfile
} from "@/api/products";
import { useAuthStore } from "@/stores/auth";
import { useTeamStore } from "@/stores/team";
import TeamRestrictedState from "@/views/team/TeamRestrictedState.vue";
import { lifecyclePresentation } from "./product-display";

/**
 * 产品列表页（P4-ORG UX 修订 / Figma P1「Product list」，含 P0 空态、
 * PX 受限态）。
 *
 * 列 = Product/key · Lifecycle · Owner · Members · Projects · Updated。
 * Owner/Members 计数来自各产品成员表（仅 Product 成员可读，非成员如实
 * 显示「—」——Discovery is not access）；Projects 计数来自各产品 Link 表。
 * 创建入口仅 Team Owner（角色即权限，服务端同矩阵强制）。
 */
const { t, locale } = useI18n();
const auth = useAuthStore();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const restricted = ref(false);
const products = ref<ProductProfile[]>([]);

interface ProductListRow {
  product: ProductProfile;
  /** 成员表可读时才有 Owner 名与成员数（非成员 = null） */
  ownerName: string | null;
  memberCount: number | null;
  projectCount: number | null;
}

const rows = ref<ProductListRow[]>([]);

const isOwner = computed(() => teamStore.teamRole === "owner");
const teamId = computed(() => teamStore.context?.team?.id ?? "");
const teamName = computed(() => teamStore.context?.team?.name ?? "");
const organization = computed(() => auth.organization ?? "");
const identityLabel = computed(() =>
  isOwner.value
    ? t("team.products.identityTeamOwner")
    : t("team.products.identityTeamMember")
);

/** Updated 列（设计稿口径：21 Nov） */
function formatUpdated(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  const tag = String(locale.value).toLowerCase().startsWith("zh")
    ? "zh-CN"
    : "en-GB";
  return new Intl.DateTimeFormat(tag, {
    day: "numeric",
    month: "short"
  }).format(date);
}

async function reload(): Promise<void> {
  if (!teamId.value) return;
  loading.value = true;
  error.value = null;
  restricted.value = false;
  try {
    const list = await listProducts(teamId.value);
    products.value = list;
    // Owner / Members / Projects 逐产品核对：成员表与 Link 表仅 Product
    // 成员可读，非成员 404 → 如实「—」（Discovery is not access）。
    rows.value = await Promise.all(
      list.map(async product => {
        try {
          const [members, links] = await Promise.all([
            listProductMembers(product.id),
            listProductLinks(product.id)
          ]);
          const owner = members.find(m => m.role === "owner" && m.is_active);
          return {
            product,
            ownerName: owner?.display_name ?? null,
            memberCount: members.filter(m => m.is_active).length,
            projectCount: links.length
          };
        } catch {
          return {
            product,
            ownerName: null,
            memberCount: null,
            projectCount: null
          };
        }
      })
    );
  } catch (err) {
    if (err instanceof HttpError && err.status === 403) {
      restricted.value = true;
    } else {
      error.value =
        err instanceof Error ? err.message : t("team.products.loadFailed");
    }
  } finally {
    loading.value = false;
  }
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
      { label: teamName, to: '/dashboard' },
      { label: $t('team.products.title') }
    ]"
    :title="$t('team.products.title')"
    :description="
      organization
        ? $t('team.products.listSubtitle', {
            team: teamName,
            org: organization
          })
        : undefined
    "
  >
    <TeamRestrictedState v-if="restricted" />

    <template v-else>
      <p v-if="error" class="tp-error">{{ error }}</p>

      <!-- 摘要行 + 创建入口（P1 顶部） -->
      <div class="tp-head">
        <span class="tp-head__summary" data-testid="products-summary">
          {{
            $t("team.products.summaryLine", {
              team: teamName,
              count: rows.length
            })
          }}
        </span>
        <Button
          v-if="isOwner"
          variant="primary"
          size="sm"
          data-test="products-create"
          @click="$router.push('/settings/team/products/create')"
        >
          {{ $t("team.products.create") }}
        </Button>
      </div>

      <!-- P0 空态 -->
      <EmptyState
        v-if="!loading && rows.length === 0"
        :title="$t('team.products.emptyTitle')"
        :description="$t('team.products.emptyHint')"
      />

      <!-- 产品表（P1） -->
      <section v-else class="tp-card" data-testid="products-card">
        <h2 class="tp-card__title">{{ $t("team.products.listTitle") }}</h2>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ $t("team.products.colProductKey") }}</TableHead>
              <TableHead>{{ $t("team.products.colLifecycle") }}</TableHead>
              <TableHead>{{ $t("team.products.colOwner") }}</TableHead>
              <TableHead>{{ $t("team.products.colMembers") }}</TableHead>
              <TableHead>{{ $t("team.products.colProjects") }}</TableHead>
              <TableHead>{{ $t("team.products.colUpdated") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="row in rows"
              :key="row.product.id"
              :data-testid="`products-row-${row.product.id}`"
            >
              <TableCell class="tp-product-name">
                <RouterLink
                  class="tp-product-link"
                  :to="`/settings/team/products/${row.product.id}`"
                >
                  {{ row.product.name }} / {{ row.product.key }}
                </RouterLink>
              </TableCell>
              <TableCell>
                <Badge
                  :variant="lifecyclePresentation(row.product.status).variant"
                >
                  {{ $t(lifecyclePresentation(row.product.status).labelKey) }}
                </Badge>
              </TableCell>
              <TableCell>{{ row.ownerName ?? "—" }}</TableCell>
              <TableCell>{{ row.memberCount ?? "—" }}</TableCell>
              <TableCell>{{ row.projectCount ?? "—" }}</TableCell>
              <TableCell>{{ formatUpdated(row.product.updated_at) }}</TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </section>

      <!-- 边界说明（P1 Info 横幅） -->
      <Alert variant="info" :title="$t('team.products.discoveryTitle')">
        {{ $t("team.products.discoveryDetail") }}
      </Alert>

      <!-- 身份脚注（P1） -->
      <p class="tp-foot" data-testid="products-identity-note">
        {{
          $t("team.products.identityFootnote", {
            identity: identityLabel,
            authority: isOwner
              ? $t("team.products.identityAuthorityOwner")
              : $t("team.products.identityAuthorityMember")
          })
        }}
      </p>
    </template>
  </PageFrame>
</template>

<style scoped>
.tp-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.tp-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.tp-head__summary {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.tp-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.tp-card__title {
  margin: 0 0 var(--space-4);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.tp-product-name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.tp-product-link {
  color: var(--text-primary);
  text-decoration: none;
}

.tp-product-link:hover {
  color: var(--ws-accent);
}

.tp-foot {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}
</style>
