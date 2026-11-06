<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Building2, UserPlus, ArrowRight } from "lucide-vue-next";

import OnboardingShell from "./OnboardingShell.vue";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";

/**
 * Screen 01 — Welcome / Team Selection（设计稿 §4）。
 * 首登双入口：创建团队 / 通过邀请加入。无 Skip 或 Later（产品裁决 Q1）。
 * 「加入」承接分享而来的邀请链接/Token → /invitation/accept。
 */
const { t } = useI18n();
const router = useRouter();

const joinToken = ref("");
const joinError = ref<string | null>(null);

function goCreate(): void {
  void router.push("/onboarding/create");
}

function goJoin(): void {
  const token = joinToken.value.trim();
  // 链接或裸 token 均可：截取 token 查询参数优先。
  const fromLink = token.match(/[?&]token=([^&]+)/);
  const raw = fromLink?.[1] ?? token;
  if (!raw) {
    joinError.value = t("team.onboarding.welcome.joinRequired");
    return;
  }
  joinError.value = null;
  void router.push({
    path: "/invitation/accept",
    query: { token: raw, type: "invite" }
  });
}
</script>

<template>
  <OnboardingShell>
    <div class="welcome">
      <h1 class="welcome__title">{{ t("team.onboarding.welcome.title") }}</h1>
      <p class="welcome__desc">{{ t("team.onboarding.welcome.desc") }}</p>

      <div class="welcome__choices">
        <button
          type="button"
          class="choice"
          data-testid="onboarding-choice-create"
          @click="goCreate"
        >
          <span class="choice__icon"><Building2 :size="22" /></span>
          <span class="choice__body">
            <span class="choice__title">{{
              t("team.onboarding.welcome.createTitle")
            }}</span>
            <span class="choice__desc">{{
              t("team.onboarding.welcome.createDesc")
            }}</span>
          </span>
          <ArrowRight class="choice__arrow" :size="16" aria-hidden="true" />
        </button>

        <div class="choice choice--static">
          <span class="choice__icon"><UserPlus :size="22" /></span>
          <span class="choice__body">
            <span class="choice__title">{{
              t("team.onboarding.welcome.joinTitle")
            }}</span>
            <span class="choice__desc">{{
              t("team.onboarding.welcome.joinDesc")
            }}</span>
            <FormField
              class="choice__field"
              :error="joinError ?? undefined"
              html-for="join-token"
            >
              <Input
                id="join-token"
                v-model="joinToken"
                data-testid="onboarding-join-token"
                :placeholder="t('team.onboarding.welcome.joinPlaceholder')"
                @keyup.enter="goJoin"
              />
            </FormField>
            <Button
              variant="secondary"
              size="sm"
              data-testid="onboarding-join-submit"
              @click="goJoin"
            >
              {{ t("team.onboarding.welcome.joinAction") }}
            </Button>
          </span>
        </div>
      </div>
    </div>
  </OnboardingShell>
</template>

<style scoped>
.welcome {
  width: 100%;
  max-width: 560px;
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  padding-top: var(--space-8);
}

.welcome__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.welcome__desc {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
}

.welcome__choices {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.choice {
  display: flex;
  align-items: flex-start;
  gap: var(--space-4);
  padding: var(--space-5);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
  text-align: left;
  cursor: pointer;
  transition: border-color var(--duration-fast) var(--ease-standard);
}

button.choice:hover {
  border-color: var(--accent);
}

.choice--static {
  cursor: default;
}

.choice__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-lg);
  height: var(--size-control-lg);
  border-radius: var(--radius-md);
  background: var(--accent-surface);
  color: var(--accent);
  flex-shrink: 0;
}

.choice__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  flex: 1;
}

.choice__title {
  font-family: var(--font-ui);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.choice__desc {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
}

.choice__field {
  margin-top: var(--space-2);
}

.choice__arrow {
  margin-top: var(--space-3);
  color: var(--text-tertiary);
  flex-shrink: 0;
}
</style>
