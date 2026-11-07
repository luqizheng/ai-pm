<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Check, CircleCheck, Copy, Download } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import Dialog from "@/components/ui/dialog/Dialog.vue";
import { registerDevice, type Device } from "@/api/devices";

/**
 * DeviceWizardDialog —— 添加执行环境向导（线稿三段式）：
 * ① 选择设备平台 → ② 安装 Runner（安装引导 / 命令）→ ③ 注册与验证（真实登记契约）。
 *
 * 契约口径（devices.ts T-088）：
 * - 登记走 POST /v1/pm/devices（首次登记缺省 device_id，由服务端生成）；
 * - 安装成功 ≠ 注册完成 ≠ Execution Ready：向导内明确提示，登记成功后
 *   仍需设备侧自检 + 绑定项目，readiness 由服务端计算。
 */
interface Props {
  open: boolean;
  /** 安装引导地址（与 devenv 一致：pm-api onboarding 页） */
  installGuideUrl?: string;
}

const props = withDefaults(defineProps<Props>(), {
  installGuideUrl: "/api/install/codex"
});

const emit = defineEmits<{
  "update:open": [value: boolean];
  /** 登记成功，携带新设备（调用方可刷新列表） */
  registered: [device: Device];
  /** 成功态点击「查看设备详情」 */
  "go-detail": [device: Device];
}>();

const { t } = useI18n();

type Platform = Device["platform"];

const STEP_COUNT = 3;
const step = ref(1);
const platform = ref<Platform>("windows");
const copied = ref(false);
const busy = ref(false);
const errorMessage = ref("");
const registered = ref<Device | null>(null);

const form = ref({
  name: "",
  os_version: "",
  executor_kind: "codex",
  executor_version: ""
});

const PLATFORMS: Platform[] = ["windows", "macos", "linux"];

watch(
  () => props.open,
  open => {
    if (open) {
      step.value = 1;
      platform.value = "windows";
      copied.value = false;
      busy.value = false;
      errorMessage.value = "";
      registered.value = null;
      form.value = {
        name: "",
        os_version: "",
        executor_kind: "codex",
        executor_version: ""
      };
    }
  }
);

const canNext = computed(() => Boolean(platform.value));

function next() {
  if (step.value < STEP_COUNT && canNext.value) {
    step.value += 1;
  }
}

function back() {
  if (step.value > 1) {
    step.value -= 1;
  }
}

function selectPlatform(value: Platform) {
  platform.value = value;
  if (step.value === 1) {
    // 线稿交互：点选平台卡即选中，等待「下一步」确认
    return;
  }
}

async function copyGuide() {
  try {
    await navigator.clipboard?.writeText(props.installGuideUrl);
    copied.value = true;
  } catch {
    copied.value = false;
  }
}

async function submit() {
  busy.value = true;
  errorMessage.value = "";
  try {
    const device = await registerDevice({
      name: form.value.name,
      platform: platform.value,
      os_version: form.value.os_version || null,
      executor_kind: form.value.executor_kind || undefined,
      executor_version: form.value.executor_version || undefined
    });
    registered.value = device;
    emit("registered", device);
  } catch (error) {
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    busy.value = false;
  }
}

function close() {
  emit("update:open", false);
}

function openDevice() {
  const device = registered.value;
  if (device) {
    emit("go-detail", device);
  }
  close();
}
</script>

<template>
  <Dialog
    :open="props.open"
    size="lg"
    :title="t('environments.wizard.title')"
    :description="t('environments.wizard.stepOf', { step, total: STEP_COUNT })"
    @update:open="emit('update:open', $event)"
  >
    <!-- 成功态 -->
    <div
      v-if="registered"
      class="env-wiz__success"
      data-testid="env-wizard-success"
    >
      <CircleCheck class="env-wiz__success-icon" aria-hidden="true" />
      <p class="env-wiz__success-title">
        {{ t("environments.wizard.successTitle") }}
      </p>
      <p class="env-wiz__success-desc">
        {{ t("environments.wizard.successDesc", { id: registered.id }) }}
      </p>
    </div>

    <!-- 步骤 ①：选择设备平台 -->
    <div
      v-else-if="step === 1"
      class="env-wiz__step"
      data-testid="env-wizard-step-1"
    >
      <p class="env-wiz__step-title">
        {{ t("environments.wizard.step1Title") }}
      </p>
      <div
        class="env-wiz__platforms"
        role="radiogroup"
        :aria-label="t('environments.wizard.step1Title')"
      >
        <button
          v-for="value in PLATFORMS"
          :key="value"
          type="button"
          role="radio"
          class="env-wiz__platform"
          :class="{ 'is-selected': platform === value }"
          :aria-checked="platform === value"
          :data-testid="`env-wizard-platform-${value}`"
          @click="selectPlatform(value)"
        >
          <span class="env-wiz__platform-name">{{
            t(`environments.platform.${value}`)
          }}</span>
          <span class="env-wiz__platform-sub">{{
            t(`environments.wizard.platformHint.${value}`)
          }}</span>
          <Check
            v-if="platform === value"
            class="env-wiz__platform-check"
            aria-hidden="true"
          />
        </button>
      </div>
    </div>

    <!-- 步骤 ②：安装 Runner -->
    <div
      v-else-if="step === 2"
      class="env-wiz__step"
      data-testid="env-wizard-step-2"
    >
      <p class="env-wiz__step-title">
        {{ t("environments.wizard.step2Title") }}
      </p>
      <p class="env-wiz__step-desc">{{ t("environments.wizard.step2Desc") }}</p>
      <div class="env-wiz__command">
        <code class="env-wiz__command-text" data-testid="env-wizard-command">
          {{ props.installGuideUrl }}
        </code>
        <button
          type="button"
          class="env-wiz__command-copy"
          data-testid="env-wizard-copy"
          @click="copyGuide"
        >
          <Copy aria-hidden="true" :size="14" />
          {{
            copied
              ? t("environments.wizard.copied")
              : t("environments.wizard.copy")
          }}
        </button>
      </div>
      <p class="env-wiz__hint">
        <Download aria-hidden="true" :size="14" />
        {{ t("environments.wizard.step2Hint") }}
      </p>
      <p class="env-wiz__warning">
        {{ t("environments.wizard.notReadyNote") }}
      </p>
    </div>

    <!-- 步骤 ③：注册与验证 -->
    <form
      v-else
      class="env-wiz__step"
      data-testid="env-wizard-step-3"
      @submit.prevent="submit"
    >
      <p class="env-wiz__step-title">
        {{ t("environments.wizard.step3Title") }}
      </p>
      <p class="env-wiz__step-desc">{{ t("environments.wizard.step3Desc") }}</p>
      <div class="env-wiz__form">
        <label class="env-wiz__field">
          <span>{{ t("environments.wizard.field.name") }}</span>
          <input
            v-model="form.name"
            type="text"
            required
            data-testid="env-wizard-name"
          />
        </label>
        <label class="env-wiz__field">
          <span>{{ t("environments.wizard.field.osVersion") }}</span>
          <input
            v-model="form.os_version"
            type="text"
            data-testid="env-wizard-os"
            :placeholder="t(`environments.wizard.platformHint.${platform}`)"
          />
        </label>
        <label class="env-wiz__field">
          <span>{{ t("environments.wizard.field.executorKind") }}</span>
          <input
            v-model="form.executor_kind"
            type="text"
            data-testid="env-wizard-kind"
          />
        </label>
        <label class="env-wiz__field">
          <span>{{ t("environments.wizard.field.executorVersion") }}</span>
          <input
            v-model="form.executor_version"
            type="text"
            data-testid="env-wizard-version"
          />
        </label>
      </div>
      <p
        v-if="errorMessage"
        class="env-wiz__error"
        data-testid="env-wizard-error"
      >
        {{ errorMessage }}
      </p>
    </form>

    <template #footer>
      <div class="env-wiz__footer">
        <Button
          v-if="!registered && step > 1"
          variant="secondary"
          size="sm"
          data-testid="env-wizard-back"
          @click="back"
        >
          {{ t("environments.wizard.back") }}
        </Button>
        <span class="env-wiz__footer-spacer" />
        <template v-if="registered">
          <Button variant="secondary" size="sm" @click="close">
            {{ t("environments.wizard.done") }}
          </Button>
          <Button variant="primary" size="sm" @click="openDevice">
            {{ t("environments.wizard.goDetail") }}
          </Button>
        </template>
        <template v-else-if="step < STEP_COUNT">
          <Button
            variant="primary"
            size="sm"
            :disabled="!canNext"
            data-testid="env-wizard-next"
            @click="next"
          >
            {{ t("environments.wizard.next") }}
          </Button>
        </template>
        <template v-else>
          <Button
            variant="primary"
            size="sm"
            :disabled="busy || !form.name"
            data-testid="env-wizard-submit"
            @click="submit"
          >
            {{ t("environments.wizard.submit") }}
          </Button>
        </template>
      </div>
    </template>
  </Dialog>
</template>

<style scoped>
.env-wiz__step {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.env-wiz__step-title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.env-wiz__step-desc {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.env-wiz__platforms {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
}

.env-wiz__platform {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-4) var(--space-3);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--ws-radius-card);
  cursor: pointer;
  text-align: center;
}

.env-wiz__platform:hover {
  border-color: var(--border-strong);
}

.env-wiz__platform:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--space-1);
}

.env-wiz__platform.is-selected {
  border-color: var(--accent);
  border-width: var(--border-2);
  background-color: var(--ws-color-accent-surface);
}

.env-wiz__platform-name {
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.env-wiz__platform-sub {
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.env-wiz__platform-check {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  color: var(--accent);
}

.env-wiz__command {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  background-color: var(--ws-canvas);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
}

.env-wiz__command-text {
  overflow: hidden;
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.env-wiz__command-copy {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--space-1);
  padding: 0;
  border: none;
  background: none;
  font-size: var(--font-size-caption);
  color: var(--accent);
  cursor: pointer;
}

.env-wiz__command-copy:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--space-1);
  border-radius: var(--radius-xs);
}

.env-wiz__hint {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.env-wiz__warning {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background-color: var(--ws-elevated);
  color: var(--ws-color-warning-text);
  font-size: var(--font-size-caption);
}

.env-wiz__form {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.env-wiz__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.env-wiz__field input {
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  background-color: var(--ws-canvas);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  font-size: var(--font-size-body);
  color: var(--ws-text-primary);
}

.env-wiz__field input:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--space-1);
}

.env-wiz__error {
  margin: 0;
  color: var(--ws-color-danger-text);
  font-size: var(--font-size-caption);
}

.env-wiz__success {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-6) 0;
  text-align: center;
}

.env-wiz__success-icon {
  width: var(--size-empty-icon);
  height: var(--size-empty-icon);
  color: var(--ws-color-success);
}

.env-wiz__success-title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.env-wiz__success-desc {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.env-wiz__footer {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
}

.env-wiz__footer-spacer {
  flex: 1;
}
</style>
