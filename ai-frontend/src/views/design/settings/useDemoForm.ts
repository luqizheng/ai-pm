import { computed, ref } from "vue";
import type { ComputedRef, Ref } from "vue";

/**
 * 设计目录演示表单（Settings Framework §6 保存/取消/错误态演示用）。
 *
 * 模拟「保存 → 短暂请求 → 成功闪烁」闭环：draft/saved 双份快照 +
 * dirty 派生；save 为假异步（约 600ms），cancel 回滚到已保存快照。
 * 只服务 `/design/settings/*` 目录页，不是业务表单模型。
 */
export interface DemoForm<T> {
  /** 当前编辑值（可自由修改） */
  draft: Ref<T>;
  /** 是否有未保存更改 */
  dirty: ComputedRef<boolean>;
  /** 保存请求进行中 */
  saving: Ref<boolean>;
  /** 刚保存成功（短暂态，自动复位） */
  saved: Ref<boolean>;
  /** 保存失败信息（演示用：非空即展示错误） */
  error: Ref<string>;
  /** 模拟保存 */
  save: () => void;
  /** 模拟失败保存（错误态演示） */
  saveWithFailure: () => void;
  /** 取消并回滚 */
  cancel: () => void;
}

export function useDemoForm<T extends object>(initial: () => T): DemoForm<T> {
  const savedSnapshot = ref<T>(initial()) as Ref<T>;
  const draft = ref<T>({ ...initial() }) as Ref<T>;
  const saving = ref(false);
  const savedFlash = ref(false);
  const error = ref("");

  const dirty = computed(
    () => JSON.stringify(draft.value) !== JSON.stringify(savedSnapshot.value)
  );

  function fakeRequest(fail: boolean) {
    saving.value = true;
    error.value = "";
    window.setTimeout(() => {
      saving.value = false;
      if (fail) {
        error.value = "error";
        return;
      }
      savedSnapshot.value = { ...draft.value };
      savedFlash.value = true;
      window.setTimeout(() => {
        savedFlash.value = false;
      }, 2000);
    }, 600);
  }

  function save() {
    fakeRequest(false);
  }

  function saveWithFailure() {
    fakeRequest(true);
  }

  function cancel() {
    draft.value = { ...savedSnapshot.value };
    error.value = "";
  }

  return {
    draft,
    dirty,
    saving,
    saved: savedFlash,
    error,
    save,
    saveWithFailure,
    cancel
  };
}
