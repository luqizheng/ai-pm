<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { SettingsFormSection } from "@/components/settings";
import PageFrame from "@/components/shell/PageFrame.vue";
import { useToast } from "@/components/ui/toast";
import { translateError } from "@/api/base";
import { listTeamMembers } from "@/api/team";
import {
  addProductMember,
  appointProductOwner,
  createProduct,
  createProductLink,
  demoteProductOwner,
  endProductLink,
  fetchProduct,
  listProductLinks,
  listProductMembers,
  listProducts,
  removeProductMember,
  setProductLifecycle,
  type ProductLink,
  type ProductMember,
  type ProductProfile
} from "@/api/products";
import { useTeamStore } from "@/stores/team";
import SettingsAppFrame from "../SettingsAppFrame.vue";
import { TEAM_APP_NAV, TEAM_ICONS } from "../nav";

/**
 * 产品管理（P4-ORG Phase 3 / DR-PA-01~03）：
 *   - 列表：Team Owner 全量；普通 Team Member 仅自己参与的（PA §6 信息路由）；
 *   - 创建（Team Owner）：指定首位 Product Owner（不因组织身份自动成为 Owner）；
 *   - 详情：成员表（Member 平权添加/移除普通成员；任命 Owner=继任指定；
 *     降级 Owner 仅 Team Owner）、生命周期按钮（任意 Product Owner）、
 *     Project 关联列表（abandoned 拦建立）。
 */
const { t } = useI18n();
const toast = useToast();
const teamStore = useTeamStore();

const loading = ref(true);
const error = ref<string | null>(null);
const products = ref<ProductProfile[]>([]);
const selectedId = ref<string>("");
const detail = ref<ProductProfile | null>(null);
const members = ref<ProductMember[]>([]);
const links = ref<ProductLink[]>([]);
const teamMembers = ref<{ principal_id: string; display_name: string }[]>([]);
const busy = ref(false);

const isOwner = computed(() => teamStore.teamRole === "owner");
const teamId = computed(() => teamStore.context?.team?.id ?? "");
const selfId = computed(() => teamStore.context?.principal_id ?? "");
const selected = computed(
  () => products.value.find(p => p.id === selectedId.value) ?? null
);

// ── 创建对话框 ───────────────────────────────────────────────
const createOpen = ref(false);
const creating = ref(false);
const draftName = ref("");
const draftDescription = ref("");
const draftOwnerId = ref("");

// ── 成员/关联操作 ────────────────────────────────────────────
const addMemberId = ref("");
const linkProjectId = ref("");

async function reload(): Promise<void> {
  if (!teamId.value) return;
  loading.value = true;
  error.value = null;
  try {
    products.value = await listProducts(teamId.value);
    // 首个产品自动选中（详情面板随选择加载）。
    if (products.value.length > 0 && !selectedId.value) {
      selectedId.value = products.value[0]!.id;
    }
    if (isOwner.value) {
      const rows = await listTeamMembers(teamId.value);
      teamMembers.value = rows
        .filter(m => m.is_active)
        .map(m => ({
          principal_id: m.principal_id,
          display_name: m.display_name
        }));
    }
    if (selectedId.value) await reloadDetail(selectedId.value);
  } catch (err) {
    error.value = translateError(err);
  } finally {
    loading.value = false;
  }
}

async function reloadDetail(productId: string): Promise<void> {
  try {
    const [profile, memberRows, linkRows] = await Promise.all([
      fetchProduct(productId),
      listProductMembers(productId),
      listProductLinks(productId)
    ]);
    detail.value = profile;
    members.value = memberRows;
    links.value = linkRows;
  } catch (err) {
    // 无 Membership 视角（Team Member 未参与）→ 详情空。
    detail.value = null;
    members.value = [];
    links.value = [];
    error.value = translateError(String(err));
  }
}

function select(id: string): void {
  selectedId.value = id;
  void reloadDetail(id);
}

async function openCreate(): Promise<void> {
  draftName.value = "";
  draftDescription.value = "";
  draftOwnerId.value = "";
  createOpen.value = true;
  await teamStore.refresh();
}

async function submitCreate(): Promise<void> {
  if (!draftName.value.trim() || !draftOwnerId.value || creating.value) return;
  creating.value = true;
  try {
    const created = await createProduct(teamId.value, {
      name: draftName.value.trim(),
      description: draftDescription.value.trim() || undefined,
      first_owner_principal_id: draftOwnerId.value
    });
    toast.toast({ title: t("team.products.createdToast") });
    createOpen.value = false;
    await reload();
    select(created.id);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    creating.value = false;
  }
}

async function submitAddMember(): Promise<void> {
  if (!addMemberId.value || !detail.value || busy.value) return;
  busy.value = true;
  try {
    await addProductMember(detail.value.id, addMemberId.value);
    addMemberId.value = "";
    toast.toast({ title: t("team.products.memberAddedToast") });
    await reloadDetail(detail.value.id);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busy.value = false;
  }
}

async function removeMember(row: ProductMember): Promise<void> {
  if (!detail.value || busy.value) return;
  busy.value = true;
  try {
    await removeProductMember(detail.value.id, row.principal_id);
    toast.toast({ title: t("team.products.memberRemovedToast") });
    await reloadDetail(detail.value.id);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busy.value = false;
  }
}

async function appoint(row: ProductMember): Promise<void> {
  if (!detail.value || busy.value) return;
  busy.value = true;
  try {
    await appointProductOwner(detail.value.id, row.principal_id);
    toast.toast({ title: t("team.products.appointedToast") });
    await reloadDetail(detail.value.id);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busy.value = false;
  }
}

async function demote(row: ProductMember): Promise<void> {
  if (!detail.value || busy.value) return;
  busy.value = true;
  try {
    await demoteProductOwner(detail.value.id, row.principal_id);
    toast.toast({ title: t("team.products.demotedToast") });
    await reloadDetail(detail.value.id);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busy.value = false;
  }
}

async function lifecycle(action: "abandon" | "restart"): Promise<void> {
  if (!detail.value || busy.value) return;
  busy.value = true;
  try {
    const updated = await setProductLifecycle(detail.value.id, action);
    detail.value = updated;
    const idx = products.value.findIndex(p => p.id === updated.id);
    if (idx >= 0) products.value[idx] = updated;
    toast.toast({ title: t(`team.products.${action}Toast`) });
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busy.value = false;
  }
}

async function submitLink(): Promise<void> {
  if (!detail.value || !linkProjectId.value.trim() || busy.value) return;
  busy.value = true;
  try {
    await createProductLink(detail.value.id, linkProjectId.value.trim());
    linkProjectId.value = "";
    toast.toast({ title: t("team.products.linkCreatedToast") });
    await reloadDetail(detail.value.id);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busy.value = false;
  }
}

async function removeLink(link: ProductLink): Promise<void> {
  if (!detail.value || busy.value) return;
  busy.value = true;
  try {
    await endProductLink(link.id);
    toast.toast({ title: t("team.products.linkEndedToast") });
    await reloadDetail(detail.value.id);
  } catch (err) {
    toast.toast({ title: translateError(err), variant: "danger" });
  } finally {
    busy.value = false;
  }
}

function canRemove(row: ProductMember): boolean {
  // 普通成员：Product Member 平权可移除；Owner 行仅 Team Owner（服务端强制）。
  return row.principal_id !== selfId.value || row.role === "member";
}

onMounted(async () => {
  await teamStore.load();
  await reload();
});
</script>

<template>
  <PageFrame
    :title="$t('team.products.title')"
    :description="$t('team.products.description')"
    :scope-label="$t('settings.team.landing.title')"
    :entries="TEAM_APP_NAV"
    :icons="TEAM_ICONS"
  >
    <template #actions>
      <Button
        v-if="isOwner"
        variant="primary"
        size="sm"
        data-test="team-products-create"
        @click="openCreate"
      >
        {{ $t("team.products.create") }}
      </Button>
    </template>

    <p v-if="error" class="tp-error">{{ error }}</p>

    <SettingsFormSection :title="$t('team.products.listTitle')">
      <p v-if="loading" class="tp-note">{{ $t("common.loading") }}</p>
      <p
        v-else-if="products.length === 0"
        class="tp-note"
        data-test="products-empty"
      >
        {{ $t("team.products.empty") }}
      </p>
      <ul v-else class="tp-list">
        <li
          v-for="p in products"
          :key="p.id"
          class="tp-item"
          :class="{ 'tp-item--active': p.id === selectedId }"
        >
          <button type="button" class="tp-item__body" @click="select(p.id)">
            <span class="tp-item__name">{{ p.name }} / {{ p.key }}</span>
            <span class="tp-item__desc">{{ p.description ?? "" }}</span>
          </button>
          <Badge :variant="p.status === 'active' ? 'success' : 'warning'">
            {{ p.status }}
          </Badge>
        </li>
      </ul>
    </SettingsFormSection>

    <!-- 详情（选中且有 Membership 时） -->
    <template v-if="detail">
      <SettingsFormSection :title="$t('team.products.membersTitle')">
        <div v-if="isOwner" class="tp-addrow">
          <select
            v-model="addMemberId"
            class="tp-select"
            data-test="product-add-member-select"
          >
            <option value="" disabled>
              {{ $t("team.products.addMemberPlaceholder") }}
            </option>
            <option
              v-for="m in teamMembers"
              :key="m.principal_id"
              :value="m.principal_id"
            >
              {{ m.display_name }}
            </option>
          </select>
          <Button
            variant="primary"
            size="sm"
            :disabled="!addMemberId || busy"
            data-test="product-add-member"
            @click="submitAddMember"
          >
            {{ $t("team.products.addMember") }}
          </Button>
        </div>
        <ul class="tp-members">
          <li v-for="m in members" :key="m.principal_id" class="tp-member">
            <span class="tp-member__name">{{ m.display_name }}</span>
            <Badge :variant="m.role === 'owner' ? 'accent' : 'neutral'">
              {{ m.role }}
            </Badge>
            <span v-if="m.principal_id === selfId" class="tp-note">
              {{ $t("team.members.you") }}
            </span>
            <span class="tp-member__spacer" />
            <Button
              v-if="m.role === 'member'"
              variant="secondary"
              size="sm"
              :disabled="busy"
              :data-test="`product-appoint-${m.principal_id}`"
              @click="appoint(m)"
            >
              {{ $t("team.products.appointOwner") }}
            </Button>
            <Button
              v-if="m.role === 'owner' && isOwner"
              variant="secondary"
              size="sm"
              :disabled="busy"
              :data-test="`product-demote-${m.principal_id}`"
              @click="demote(m)"
            >
              {{ $t("team.products.demoteOwner") }}
            </Button>
            <Button
              v-if="canRemove(m)"
              variant="danger"
              size="sm"
              :disabled="busy"
              :data-test="`product-remove-${m.principal_id}`"
              @click="removeMember(m)"
            >
              {{ $t("team.members.remove") }}
            </Button>
          </li>
        </ul>
      </SettingsFormSection>

      <SettingsFormSection :title="$t('team.products.lifecycleTitle')">
        <div class="tp-actions">
          <Badge :variant="detail.status === 'active' ? 'success' : 'warning'">
            {{ detail.status }}
          </Badge>
          <Button
            v-if="detail.status === 'active'"
            variant="danger"
            size="sm"
            :disabled="busy"
            data-test="product-abandon"
            @click="lifecycle('abandon')"
          >
            {{ $t("team.products.abandon") }}
          </Button>
          <Button
            v-else
            variant="primary"
            size="sm"
            :disabled="busy"
            data-test="product-restart"
            @click="lifecycle('restart')"
          >
            {{ $t("team.products.restart") }}
          </Button>
        </div>
        <p class="tp-note">{{ $t("team.products.lifecycleNote") }}</p>
      </SettingsFormSection>

      <SettingsFormSection :title="$t('team.products.linksTitle')">
        <ul class="tp-links">
          <li v-for="l in links" :key="l.id" class="tp-member">
            <span class="tp-member__name"
              >{{ $t("team.products.linkProject") }}
              {{ l.project_id.slice(0, 8) }}</span
            >
            <span class="tp-member__spacer" />
            <Button
              v-if="isOwner"
              variant="danger"
              size="sm"
              :disabled="busy"
              :data-test="`product-link-end-${l.id}`"
              @click="removeLink(l)"
            >
              {{ $t("team.products.linkEnd") }}
            </Button>
          </li>
        </ul>
        <div v-if="isOwner && detail.status === 'active'" class="tp-addrow">
          <Input
            v-model="linkProjectId"
            :placeholder="$t('team.products.linkPlaceholder')"
            data-test="product-link-project"
          />
          <Button
            variant="primary"
            size="sm"
            :disabled="!linkProjectId.trim() || busy"
            data-test="product-link-create"
            @click="submitLink"
          >
            {{ $t("team.products.linkCreate") }}
          </Button>
        </div>
      </SettingsFormSection>
    </template>

    <!-- 创建对话框（Team Owner） -->
    <Dialog
      :open="createOpen"
      :title="$t('team.products.createTitle')"
      :description="$t('team.products.createDesc')"
      size="sm"
      @update:open="
        value => {
          createOpen = value;
        }
      "
    >
      <FormField :label="$t('team.onboarding.create.nameLabel')" required>
        <Input
          v-model="draftName"
          :maxlength="120"
          data-test="product-create-name"
        />
      </FormField>
      <FormField :label="$t('team.onboarding.create.descLabel')">
        <Textarea
          v-model="draftDescription"
          :rows="3"
          data-test="product-create-desc"
        />
      </FormField>
      <FormField :label="$t('team.products.firstOwnerLabel')" required>
        <select
          v-model="draftOwnerId"
          class="tp-select"
          data-test="product-create-owner"
        >
          <option value="" disabled>
            {{ $t("team.products.firstOwnerPlaceholder") }}
          </option>
          <option
            v-for="m in teamMembers"
            :key="m.principal_id"
            :value="m.principal_id"
          >
            {{ m.display_name }}
          </option>
        </select>
      </FormField>
      <template #footer>
        <Button variant="secondary" size="sm" @click="createOpen = false">
          {{ $t("team.members.cancel") }}
        </Button>
        <Button
          variant="primary"
          size="sm"
          :loading="creating"
          data-test="product-create-confirm"
          @click="submitCreate"
        >
          {{ $t("team.products.create") }}
        </Button>
      </template>
    </Dialog>
  </PageFrame>
</template>

<style scoped>
.tp-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.tp-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.tp-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.tp-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
}

.tp-item--active {
  border-color: var(--ws-color-accent);
}

.tp-item__body {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-1);
  border: 0;
  background: none;
  cursor: pointer;
  padding: 0;
  text-align: left;
}

.tp-item__name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.tp-item__desc {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.tp-addrow {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-3);
}

.tp-select {
  min-width: 220px;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
}

.tp-members,
.tp-links {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.tp-member {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) 0;
}

.tp-member__name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.tp-member__spacer {
  flex: 1;
}

.tp-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
</style>
