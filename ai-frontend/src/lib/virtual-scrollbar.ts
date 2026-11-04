/**
 * 虚拟滚动条（Figma 式 overlay scrollbar）—— 全系统统一滚动行为。
 *
 * 与 styles/index.css 的 `.vsb-*` 样式配合：原生滚动条已全局隐藏（零占位），
 * 本模块在 body 直挂的固定层里为每个滚动容器渲染 overlay thumb。
 *
 * 设计要点：
 * - 零逐组件接入：MutationObserver 扫描全文档 overflow:auto/scroll 的元素
 *   自动跟踪；新增节点（含 reka-ui 传送门弹层）在下一帧纳入。
 * - Figma 交互口径：常态隐藏；悬停容器或滚动中淡入，静止约 700ms 后淡出；
 *   thumb 可拖拽，拖拽中加深并保持可见；滚轮落在 thumb 上时转发给容器。
 * - 只跟踪「滚动意图」（overflow 为 auto/scroll/overlay，html 根滚动器单独
 *   放行），与原生滚动条同判据；内容不足一屏时 thumb 自动隐藏。
 * - 豁免：滚动容器加 data-vsb-ignore 即不渲染 thumb。
 * - 帧内先读后写：几何读取集中在快照阶段，避免读写交错引发布局抖动。
 */

/** 滚动静止多少毫秒后淡出（Figma 口径约 1s 内） */
const SCROLL_IDLE_MS = 700;
/** thumb 最短长度，避免长列表下滑块小到无法拖拽 */
const MIN_THUMB_PX = 24;
/** thumb 与容器边缘的内缩 */
const EDGE_INSET_PX = 2;
/** 两端各留出 EDGE_INSET，thumb 可滑动的轨道长度 */
const TRACK_INSET_PX = EDGE_INSET_PX * 2;

export interface ThumbGeometry {
  /** thumb 起点相对容器内容盒（已含边缘内缩） */
  start: number;
  /** thumb 长度 */
  size: number;
}

/**
 * 纯几何：由滚动指标计算 thumb 在轨道上的位置与长度。
 * 内容不足一屏（含 1px 容差）返回 null，即「不可滚动、不显示」。
 */
export function computeThumbGeometry(
  trackSize: number,
  scrollSize: number,
  scrollPos: number
): ThumbGeometry | null {
  if (scrollSize <= trackSize + 1) return null;
  const travelSize = trackSize - TRACK_INSET_PX;
  const rawSize = (travelSize * trackSize) / scrollSize;
  const size = Math.min(Math.max(rawSize, MIN_THUMB_PX), travelSize);
  const maxScroll = scrollSize - trackSize;
  const pos = Math.min(Math.max(scrollPos, 0), maxScroll);
  const travel = travelSize - size;
  return { start: EDGE_INSET_PX + (pos / maxScroll) * travel, size };
}

interface TrackRecord {
  el: HTMLElement;
  vertical: HTMLDivElement;
  horizontal: HTMLDivElement;
  /** 指针悬停在容器（或其 thumb）上 */
  hovered: boolean;
  /** thumb 拖拽中 */
  dragging: boolean;
  /** 最近一次滚动/拖拽的保活截止时刻（performance.now 时钟） */
  activeUntil: number;
}

export interface VirtualScrollbarHandle {
  uninstall(): void;
}

/** 模块级单例：entry 重复执行（dev HMR）时复用同一份安装 */
let activeHandle: VirtualScrollbarHandle | null = null;

export function installVirtualScrollbars(): VirtualScrollbarHandle {
  if (activeHandle) return activeHandle;

  const layer = document.createElement("div");
  layer.className = "vsb-layer";
  layer.setAttribute("aria-hidden", "true");
  document.body.appendChild(layer);

  /** 容器 → 跟踪记录 */
  const records = new Map<HTMLElement, TrackRecord>();
  /** thumb 元素 → 所属记录 + 方向（拖拽/hover 事件反查用） */
  const thumbIndex = new Map<
    HTMLDivElement,
    { record: TrackRecord; axis: "v" | "h" }
  >();

  let rescanScheduled = false;
  let loopRunning = false;
  let dirty = false;

  function track(el: HTMLElement): void {
    if (records.has(el)) return;
    const vertical = document.createElement("div");
    vertical.className = "vsb-thumb vsb-thumb--vertical";
    const horizontal = document.createElement("div");
    horizontal.className = "vsb-thumb vsb-thumb--horizontal";
    const record: TrackRecord = {
      el,
      vertical,
      horizontal,
      hovered: false,
      dragging: false,
      activeUntil: 0
    };
    for (const [thumb, axis] of [
      [vertical, "v"],
      [horizontal, "h"]
    ] as const) {
      thumbIndex.set(thumb, { record, axis });
      // thumb 悬浮在容器上方会截住滚轮，转发给容器
      thumb.addEventListener("wheel", e => {
        e.preventDefault();
        if (axis === "v") record.el.scrollTop += e.deltaY;
        else record.el.scrollLeft += e.deltaY;
      });
      thumb.addEventListener("pointerdown", e => {
        startDrag(record, axis, thumb, e);
      });
      layer.appendChild(thumb);
    }
    records.set(el, record);
    dirty = true;
    ensureLoop();
  }

  function untrack(el: HTMLElement): void {
    const record = records.get(el);
    if (!record) return;
    thumbIndex.delete(record.vertical);
    thumbIndex.delete(record.horizontal);
    record.vertical.remove();
    record.horizontal.remove();
    records.delete(el);
  }

  /** 滚动意图判据（与原生滚动条一致的 overflow 取值） */
  function hasScrollIntent(value: string): boolean {
    return value === "auto" || value === "scroll" || value === "overlay";
  }

  /** 全文档重扫：跟踪新的滚动容器，清掉已断开的。rAF 合并，一帧最多一次 */
  function rescan(): void {
    for (const el of document.body.querySelectorAll<HTMLElement>("*")) {
      if (records.has(el)) continue;
      if (el.closest(".vsb-layer")) continue;
      if (el.hasAttribute("data-vsb-ignore")) continue;
      const cs = getComputedStyle(el);
      if (hasScrollIntent(cs.overflowY) || hasScrollIntent(cs.overflowX)) {
        track(el);
      }
    }
    for (const el of [...records.keys()]) {
      if (!el.isConnected) untrack(el);
    }
    // html 根滚动器：overflow 是 visible 时 UA 也会出滚动条，按实际可滚放行
    const root = document.scrollingElement;
    if (
      root instanceof HTMLElement &&
      root.scrollHeight > root.clientHeight + 1
    ) {
      track(root);
    }
    dirty = true;
  }

  function scheduleRescan(): void {
    if (rescanScheduled) return;
    rescanScheduled = true;
    requestAnimationFrame(() => {
      rescanScheduled = false;
      rescan();
    });
  }

  /** 可见判据：悬停中 / 拖拽中 / 滚动保活期内 */
  function isVisible(record: TrackRecord, now: number): boolean {
    return record.hovered || record.dragging || now < record.activeUntil;
  }

  interface Snapshot {
    record: TrackRecord;
    rect: DOMRect;
    vertical: ThumbGeometry | null;
    horizontal: ThumbGeometry | null;
    show: boolean;
    /** thumb 自身厚度（含主题 token 的 8px；读阶段统一取） */
    vBar: number;
    hBar: number;
  }

  function snapshot(now: number): Snapshot[] {
    const out: Snapshot[] = [];
    for (const record of records.values()) {
      const rect = record.el.getBoundingClientRect();
      out.push({
        record,
        rect,
        vertical: computeThumbGeometry(
          rect.height,
          record.el.scrollHeight,
          record.el.scrollTop
        ),
        horizontal: computeThumbGeometry(
          rect.width,
          record.el.scrollWidth,
          record.el.scrollLeft
        ),
        show: isVisible(record, now),
        vBar: record.vertical.offsetWidth,
        hBar: record.horizontal.offsetHeight
      });
    }
    return out;
  }

  function placeThumb(
    thumb: HTMLDivElement,
    geometry: ThumbGeometry | null,
    show: boolean,
    place: (g: ThumbGeometry) => { left: number; top: number; size: number }
  ): void {
    if (!geometry) {
      thumb.classList.remove("is-visible");
      return;
    }
    const { left, top, size } = place(geometry);
    // 值未变不写 style，避免无效样式失效
    if (thumb.style.left !== `${left}px`) thumb.style.left = `${left}px`;
    if (thumb.style.top !== `${top}px`) thumb.style.top = `${top}px`;
    if (thumb.classList.contains("vsb-thumb--vertical")) {
      if (thumb.style.height !== `${size}px`) thumb.style.height = `${size}px`;
    } else if (thumb.style.width !== `${size}px`) {
      thumb.style.width = `${size}px`;
    }
    thumb.classList.toggle("is-visible", show);
  }

  /** 中央 rAF 循环：只在可能有 thumb 可见、正在淡出或有待处理脏标记时运转，
   *  空闲零开销。淡出后（is-visible 摘除）由 CSS 过渡收尾，循环即停。 */
  function tick(now: number): void {
    loopRunning = false;
    const all = [...records.values()];
    const anyVisible = all.some(r => isVisible(r, now));
    // 有 thumb 仍处于显示态（含保活刚过期的淡出帧）时必须继续刷写，
    // 否则过期后没有任何事件再触发 tick，thumb 会永远停留在屏幕上
    const anyShown = all.some(
      r =>
        r.vertical.classList.contains("is-visible") ||
        r.horizontal.classList.contains("is-visible")
    );
    if (dirty || anyVisible || anyShown) {
      dirty = false;
      // 先读后写：快照阶段统一读几何，写阶段统一落 DOM
      for (const s of snapshot(now)) {
        placeThumb(s.record.vertical, s.vertical, s.show, g => ({
          left: s.rect.right - EDGE_INSET_PX - s.vBar,
          top: s.rect.top + g.start,
          size: g.size
        }));
        placeThumb(s.record.horizontal, s.horizontal, s.show, g => ({
          left: s.rect.left + g.start,
          top: s.rect.bottom - EDGE_INSET_PX - s.hBar,
          size: g.size
        }));
      }
    }
    if (anyVisible) ensureLoop();
  }

  function ensureLoop(): void {
    if (loopRunning) return;
    loopRunning = true;
    requestAnimationFrame(tick);
  }

  /** 任意滚动（捕获阶段收全文档）：保活 + 下一帧刷新全部可见 thumb
   *  （嵌套滚动容器被外层推动时矩形会位移，一并跟随）
   *  根滚动器的 scroll 事件 target 是 Document，映射回 scrollingElement */
  function onScroll(e: Event): void {
    const target = e.target === document ? document.scrollingElement : e.target;
    if (!(target instanceof HTMLElement)) return;
    const record = records.get(target);
    if (!record) return;
    record.activeUntil = performance.now() + SCROLL_IDLE_MS;
    dirty = true;
    ensureLoop();
  }

  function findRecordByTarget(target: EventTarget | null): TrackRecord | null {
    if (!(target instanceof Node)) return null;
    for (let node: Node | null = target; node; node = node.parentNode) {
      if (node instanceof HTMLElement) {
        // thumb 在 body 直挂层里、DOM 上不在容器内，先查 thumb 反查表
        const byThumb = thumbIndex.get(node as HTMLDivElement);
        if (byThumb) return byThumb.record;
        const record = records.get(node);
        if (record) return record;
      }
    }
    return null;
  }

  function onPointerOver(e: Event): void {
    const record = findRecordByTarget(e.target);
    if (!record) return;
    record.hovered = true;
    dirty = true;
    ensureLoop();
  }

  function onPointerOut(e: Event): void {
    const record = findRecordByTarget(e.target);
    if (!record || !record.hovered) return;
    // relatedTarget 仍在同一容器/thumb 内则不算离开（子元素间移动会触发 out）
    const to = (e as PointerEvent).relatedTarget;
    if (findRecordByTarget(to) === record) return;
    record.hovered = false;
    dirty = true;
    ensureLoop();
  }

  function startDrag(
    record: TrackRecord,
    axis: "v" | "h",
    thumb: HTMLDivElement,
    down: PointerEvent
  ): void {
    down.preventDefault();
    record.dragging = true;
    record.activeUntil = performance.now() + SCROLL_IDLE_MS;
    thumb.classList.add("is-dragging");
    // 指针失效（合成事件、极快速点按）时捕获会抛错；拖拽不依赖捕获也能工作
    try {
      thumb.setPointerCapture(down.pointerId);
    } catch {
      /* 无捕获继续，move/up 兜底挂在 window */
    }

    const el = record.el;
    const startPointer = axis === "v" ? down.clientY : down.clientX;
    const startScroll = axis === "v" ? el.scrollTop : el.scrollLeft;
    const rect = el.getBoundingClientRect();
    const trackSize = axis === "v" ? rect.height : rect.width;
    const scrollSize = axis === "v" ? el.scrollHeight : el.scrollWidth;
    // thumb 可滑动距离 ↔ 内容可滚动距离 的比例（按下瞬间冻结）
    const thumbLength =
      computeThumbGeometry(trackSize, scrollSize, startScroll)?.size ?? 1;
    const scrollRange = scrollSize - trackSize;
    const thumbTravel = Math.max(trackSize - TRACK_INSET_PX - thumbLength, 1);
    const ratio = scrollRange / thumbTravel;

    const onMove = (e: PointerEvent): void => {
      if (!record.dragging) return;
      const delta = (axis === "v" ? e.clientY : e.clientX) - startPointer;
      if (axis === "v") el.scrollTop = startScroll + delta * ratio;
      else el.scrollLeft = startScroll + delta * ratio;
    };
    const onUp = (e: PointerEvent): void => {
      record.dragging = false;
      record.activeUntil = performance.now() + SCROLL_IDLE_MS;
      thumb.classList.remove("is-dragging");
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      if (thumb.hasPointerCapture(e.pointerId)) {
        thumb.releasePointerCapture(e.pointerId);
      }
      dirty = true;
      ensureLoop();
    };
    // 挂 window：有捕获时事件仍会冒泡到 window；无捕获（兜底）也能收到
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function onWindowResize(): void {
    dirty = true;
    ensureLoop();
  }

  const mutation = new MutationObserver(scheduleRescan);
  mutation.observe(document.documentElement, {
    childList: true,
    subtree: true,
    attributes: true
  });
  document.addEventListener("scroll", onScroll, {
    capture: true,
    passive: true
  });
  document.addEventListener("pointerover", onPointerOver, { capture: true });
  document.addEventListener("pointerout", onPointerOut, { capture: true });
  window.addEventListener("resize", onWindowResize);

  rescan();

  const handle: VirtualScrollbarHandle = {
    uninstall() {
      mutation.disconnect();
      document.removeEventListener("scroll", onScroll, { capture: true });
      document.removeEventListener("pointerover", onPointerOver, {
        capture: true
      });
      document.removeEventListener("pointerout", onPointerOut, {
        capture: true
      });
      window.removeEventListener("resize", onWindowResize);
      for (const el of [...records.keys()]) untrack(el);
      layer.remove();
      if (activeHandle === handle) activeHandle = null;
    }
  };
  activeHandle = handle;
  return handle;
}
