// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  computeThumbGeometry,
  installVirtualScrollbars,
  type VirtualScrollbarHandle
} from "./virtual-scrollbar";

/**
 * happy-dom 无真实布局：滚动指标（scrollHeight/scrollTop）与容器矩形
 * 一律用 defineProperty / mock 桩替换，走真实的扫描→滚动→显隐管线。
 */
function stubScrollMetrics(
  el: HTMLElement,
  metrics: {
    rect: {
      top: number;
      left: number;
      right: number;
      bottom: number;
      width: number;
      height: number;
    };
    scrollHeight?: number;
    scrollWidth?: number;
    scrollTop?: number;
    scrollLeft?: number;
  }
): void {
  vi.spyOn(el, "getBoundingClientRect").mockReturnValue(
    metrics.rect as DOMRect
  );
  for (const key of [
    "scrollHeight",
    "scrollWidth",
    "scrollTop",
    "scrollLeft"
  ] as const) {
    if (metrics[key] !== undefined) {
      Object.defineProperty(el, key, {
        configurable: true,
        value: metrics[key]
      });
    }
  }
}

/** 等 rAF 帧（happy-dom 的 rAF 走宏任务队列） */
function frame(): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, 30));
}

function install(): VirtualScrollbarHandle {
  document.body.innerHTML = "";
  return installVirtualScrollbars();
}

describe("computeThumbGeometry 纯几何", () => {
  it("内容不足一屏返回 null（不可滚动、不显示）", () => {
    expect(computeThumbGeometry(400, 400, 0)).toBeNull();
    // 1px 容差
    expect(computeThumbGeometry(400, 401, 0)).toBeNull();
  });

  it("按滚动比例给出位置与长度", () => {
    const g = computeThumbGeometry(400, 800, 200);
    expect(g).not.toBeNull();
    // 轨道 400，内缩 4 → 可滑 396；thumb = 396*400/800 = 198
    expect(g!.size).toBeCloseTo(198, 5);
    // 滚到一半：起点 = 2 + (200/400)*(396-198) = 2 + 99 = 101
    expect(g!.start).toBeCloseTo(101, 5);
  });

  it("scrollPos 越界收敛到 [0, maxScroll]，thumb 长度不小于最小值", () => {
    const g = computeThumbGeometry(400, 10000, 99999);
    expect(g!.size).toBeGreaterThanOrEqual(24);
    // thumb 不得滑出轨道底
    expect(g!.start + g!.size).toBeLessThanOrEqual(400);
    const top = computeThumbGeometry(400, 10000, -50);
    expect(top!.start).toBeCloseTo(2, 5);
  });
});

describe("虚拟滚动条控制器", () => {
  let handle: VirtualScrollbarHandle;

  afterEach(() => {
    handle?.uninstall();
    handle = undefined as unknown as VirtualScrollbarHandle;
    document.body.innerHTML = "";
    vi.restoreAllMocks();
  });

  it("安装后在 body 直挂固定层，卸载时移除", () => {
    handle = install();
    const layer = document.body.querySelector(".vsb-layer");
    expect(layer).not.toBeNull();
    expect(layer!.parentElement).toBe(document.body);

    handle.uninstall();
    expect(document.body.querySelector(".vsb-layer")).toBeNull();
  });

  it("扫描跟踪 overflow:auto 容器：滚动后竖向 thumb 浮出，静止后淡出", async () => {
    handle = install();
    const el = document.createElement("div");
    el.style.overflowY = "auto";
    el.style.height = "100px";
    document.body.appendChild(el);
    // 新节点在下一帧被 MutationObserver 纳入
    await frame();

    stubScrollMetrics(el, {
      rect: {
        top: 0,
        left: 0,
        right: 300,
        bottom: 100,
        width: 300,
        height: 100
      },
      scrollHeight: 1000,
      scrollTop: 250
    });
    // 滚动事件（捕获到 document）
    el.dispatchEvent(new Event("scroll"));
    await frame();

    const layer = document.body.querySelector(".vsb-layer")!;
    const vertical = layer.querySelector<HTMLElement>(".vsb-thumb--vertical")!;
    expect(vertical.classList.contains("is-visible")).toBe(true);
    // 贴容器右缘内缩：left = right - 2 - thumb 自身厚度（happy-dom 无布局，动态取）
    expect(parseInt(vertical.style.left, 10)).toBe(
      300 - 2 - vertical.offsetWidth
    );
    // thumb = (100-4)*100/1000 = 9.6 → 最小 24；top = 2 + (250/750)*(96-24)
    expect(vertical.style.height).toBe("24px");
    expect(parseInt(vertical.style.top, 10)).toBeGreaterThan(0);
    expect(parseInt(vertical.style.top, 10)).toBeLessThan(100);

    // 静止超过 700ms 后淡出（is-visible 摘除，CSS 过渡收尾）
    await new Promise(resolve => setTimeout(resolve, 820));
    await frame();
    expect(vertical.classList.contains("is-visible")).toBe(false);
  });

  it("hover 容器即显示 thumb，离开后淡出", async () => {
    handle = install();
    const el = document.createElement("div");
    el.style.overflowY = "auto";
    document.body.appendChild(el);
    await frame();

    stubScrollMetrics(el, {
      rect: {
        top: 0,
        left: 0,
        right: 300,
        bottom: 100,
        width: 300,
        height: 100
      },
      scrollHeight: 1000
    });

    el.dispatchEvent(
      new MouseEvent("pointerover", { bubbles: true, composed: true })
    );
    await frame();
    const vertical = document.body.querySelector<HTMLElement>(
      ".vsb-thumb--vertical"
    )!;
    expect(vertical.classList.contains("is-visible")).toBe(true);

    el.dispatchEvent(
      new MouseEvent("pointerout", {
        bubbles: true,
        composed: true,
        relatedTarget: null
      })
    );
    await frame();
    expect(vertical.classList.contains("is-visible")).toBe(false);
  });

  it("data-vsb-ignore 容器不渲染 thumb", async () => {
    handle = install();
    const el = document.createElement("div");
    el.style.overflowY = "auto";
    el.setAttribute("data-vsb-ignore", "");
    document.body.appendChild(el);
    await frame();

    expect(document.body.querySelector(".vsb-thumb--vertical")).toBeNull();
  });

  it("卸载后停止跟踪且 thumb 全部移除", async () => {
    handle = install();
    const el = document.createElement("div");
    el.style.overflowY = "auto";
    document.body.appendChild(el);
    await frame();
    expect(document.body.querySelectorAll(".vsb-thumb").length).toBeGreaterThan(
      0
    );

    handle.uninstall();
    handle = undefined as unknown as VirtualScrollbarHandle;
    expect(document.body.querySelectorAll(".vsb-thumb").length).toBe(0);
    // 重复卸载安全
    expect(() => handle?.uninstall()).not.toThrow();
  });
});
