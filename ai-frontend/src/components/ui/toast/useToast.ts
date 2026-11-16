import { ref } from "vue";

export interface ToastOptions {
  variant?: "success" | "info" | "warning" | "danger";
  title: string;
  description?: string;
  closable?: boolean;
  closeLabel?: string;
  duration?: number;
}

/**
 * Toast 应用级单例状态（G-UI-1）。
 *
 * 状态放模块级而非 provide/inject：ToastHost 挂在根组件，与页面是兄弟
 * 而非祖先，provide 覆盖不到页面子树（此前因此静默 no-op）。
 * 同屏最多 1 条，新 toast 替换旧 toast（Phase 2 §6.3）。
 */
export interface ToastApi {
  toast(options: ToastOptions): void;
  dismiss(): void;
}

const current = ref<ToastOptions | null>(null);
const seq = ref(0);

/** ToastHost 渲染用的状态（应用内单例，勿在业务代码直接赋值）。 */
export function useToastState() {
  return { current, seq };
}

export const toastApi: ToastApi = {
  toast(options: ToastOptions) {
    current.value = { ...options };
    seq.value += 1;
  },
  dismiss() {
    current.value = null;
  }
};

/**
 * Toast 调用入口。
 *
 * 页面通过 `const { toast } = useToast()` 触发提示；
 * 状态为应用级单例，与 ToastHost 挂载位置、挂载与否无关。
 */
export function useToast(): ToastApi {
  return toastApi;
}
