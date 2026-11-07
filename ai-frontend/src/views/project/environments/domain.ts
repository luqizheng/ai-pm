import type { Device, DeviceReadiness } from "@/api/devices";

/**
 * 执行环境（Execution Environments）视图层派生。
 *
 * 数据权威仍是 devices.ts 契约（T-088，pm-api `/v1/pm/devices*`）：
 * readiness 是服务端计算快照（checks.rs compute_readiness），本文件只做
 * **展示派生**（阶段链 / 异常文案键 / 指标 / 健康行），不做客户端自检推断、
 * 不渲染后端没有的状态源（沿用 devenv 的「不渲染假状态」口径）。
 */

/** 阶段链第 4 位 = 终态（就绪 / 降级 / 失败 / 未自检） */
export type ChainStepKey =
  | "installed"
  | "connected"
  | "authenticated"
  | "terminal";

export type ChainStepState =
  | "pass"
  | "warning"
  | "fail"
  | "pending"
  | "unknown";

export interface EnvChainStep {
  key: ChainStepKey;
  state: ChainStepState;
  /** i18n key（environments.chain.*），由调用方翻译 */
  labelKey: string;
}

const READINESS_RANK: Partial<Record<DeviceReadiness, number>> = {
  installed: 1,
  connected: 2,
  authenticated: 3,
  ready: 4
};

function evidenceFlag(
  device: Device,
  flag: "installed" | "connected"
): boolean | null {
  const value = device.last_check?.[flag];
  return typeof value === "boolean" ? value : null;
}

/** 就绪快照 → 阶段链（Installed → Connected → Authenticated → 终态） */
export function readinessChainSteps(device: Device): EnvChainStep[] {
  const rank = device.readiness ? (READINESS_RANK[device.readiness] ?? 0) : 0;

  function step(
    key: ChainStepKey,
    flag: boolean | null,
    stepRank: number,
    labelKey: string,
    negativeLabelKey: string
  ): EnvChainStep {
    if (flag === true || (flag === null && stepRank <= rank && rank > 0)) {
      return { key, state: "pass", labelKey };
    }
    if (flag === false) {
      return { key, state: "fail", labelKey: negativeLabelKey };
    }
    // 无证据且快照未推进到该步：未自检（全灰）或等待中
    if (rank === 0) {
      return { key, state: "unknown", labelKey };
    }
    return { key, state: "pending", labelKey };
  }

  return [
    step(
      "installed",
      evidenceFlag(device, "installed"),
      1,
      "environments.chain.installed",
      "environments.chain.notInstalled"
    ),
    step(
      "connected",
      evidenceFlag(device, "connected"),
      2,
      "environments.chain.connected",
      "environments.chain.disconnected"
    ),
    step(
      "authenticated",
      null,
      3,
      "environments.chain.authenticated",
      "environments.chain.authPending"
    ),
    terminalStep(device)
  ];
}

function terminalStep(device: Device): EnvChainStep {
  if (device.status === "revoked") {
    return {
      key: "terminal",
      state: "fail",
      labelKey: "environments.chain.revoked"
    };
  }
  switch (device.readiness) {
    case "ready":
      return {
        key: "terminal",
        state: "pass",
        labelKey: "environments.chain.ready"
      };
    case "degraded":
      return {
        key: "terminal",
        state: "warning",
        labelKey: "environments.chain.degraded"
      };
    case "failed":
      return {
        key: "terminal",
        state: "fail",
        labelKey: "environments.chain.failed"
      };
    case null:
      return {
        key: "terminal",
        state: "unknown",
        labelKey: "environments.chain.unchecked"
      };
    default:
      return {
        key: "terminal",
        state: "pending",
        labelKey: "environments.chain.pending"
      };
  }
}

/** 卡片异常行文案键；null = 无异常行 */
export function anomalyKey(device: Device): string | null {
  if (device.status === "revoked") return "environments.anomaly.revoked";
  if (device.readiness === "degraded") return "environments.anomaly.degraded";
  if (device.readiness === "failed") return "environments.anomaly.failed";
  return null;
}

export interface EnvMetrics {
  total: number;
  ready: number;
  trouble: number;
}

/** 指标三件套：设备总数 / Execution Ready / 异常设备 */
export function envMetrics(devices: Device[]): EnvMetrics {
  return {
    total: devices.length,
    ready: devices.filter(device => device.readiness === "ready").length,
    trouble: devices.filter(
      device =>
        device.status === "revoked" ||
        device.readiness === "degraded" ||
        device.readiness === "failed"
    ).length
  };
}

export interface EnvFilters {
  /** 关键字：匹配设备 ID / 名称 / 系统版本 / 执行器 */
  query: string;
  /** all | ready | degraded | failed | unchecked | revoked */
  status: string;
  /** all | windows | macos | linux */
  platform: string;
}

/** 列表过滤（纯函数，便于单测）：状态 / 平台 / 关键字 */
export function filterDevices(
  devices: Device[],
  filters: EnvFilters
): Device[] {
  const keyword = filters.query.trim().toLowerCase();
  return devices.filter(device => {
    if (filters.status === "revoked") {
      if (device.status !== "revoked") return false;
    } else if (
      filters.status !== "all" &&
      device.readiness !== filters.status
    ) {
      return false;
    }
    if (filters.platform !== "all" && device.platform !== filters.platform) {
      return false;
    }
    if (!keyword) return true;
    return [device.id, device.name, device.os_version, device.executor_kind]
      .filter(Boolean)
      .some(field => String(field).toLowerCase().includes(keyword));
  });
}

/** readiness → Badge / 状态语义 */
export function readinessVariant(
  device: Device
): "success" | "warning" | "danger" | "neutral" {
  if (device.status === "revoked") return "danger";
  if (device.readiness === "ready") return "success";
  if (device.readiness === "degraded" || device.readiness === "failed") {
    return "danger";
  }
  if (device.readiness) return "warning";
  return "neutral";
}

/** readiness → i18n 标签键（environments.readiness.*） */
export function readinessLabelKey(device: Device): string {
  if (device.status === "revoked") return "environments.readiness.revoked";
  if (!device.readiness) return "environments.readiness.unchecked";
  return `environments.readiness.${device.readiness}`;
}

// ─── 相对时间（最后自检）───────────────────────────────────────────────

export interface RelativeTimeKey {
  /** i18n key（environments.time.*） */
  key: string;
  /** 插值参数 */
  n?: number;
}

/** 最近自检时间 → 相对时间文案键（justNow / minutesAgo / hoursAgo / daysAgo） */
export function checkRelativeTime(
  iso: string | null | undefined,
  now: number = Date.now()
): RelativeTimeKey {
  if (!iso) return { key: "environments.time.never" };
  const then = Date.parse(iso);
  if (Number.isNaN(then)) return { key: "environments.time.never" };
  const diffMinutes = Math.max(0, Math.floor((now - then) / 60000));
  if (diffMinutes < 1) return { key: "environments.time.justNow" };
  if (diffMinutes < 60)
    return { key: "environments.time.minutesAgo", n: diffMinutes };
  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24)
    return { key: "environments.time.hoursAgo", n: diffHours };
  return { key: "environments.time.daysAgo", n: Math.floor(diffHours / 24) };
}

// ─── 详情页健康检查行 ─────────────────────────────────────────────────

export type HealthState = "pass" | "warning" | "fail" | "unknown";

export interface EnvHealthRow {
  key: "installed" | "connected" | "tool" | "repository" | "authentication";
  state: HealthState;
  /** 仓库验证覆盖文案参数（verified / declared） */
  coverage?: { verified: number; declared: number };
}

/**
 * 详情页「运行健康检查」。
 *
 * 5 行中前 3 行来自最近自检证据（installed / connected / tool），
 * 仓库权限行来自 verified/declared 仓库计数，Authentication 行从 readiness
 * 快照派生（无独立状态源，仅 authenticated/ready 视为通过，其余不推断）。
 */
export function healthRows(device: Device): EnvHealthRow[] {
  const lastCheck = device.last_check ?? {};
  const rank = device.readiness ? (READINESS_RANK[device.readiness] ?? 0) : 0;

  function flagRow(
    key: EnvHealthRow["key"],
    flag: boolean | undefined,
    impliedBySnapshot: boolean
  ): EnvHealthRow {
    if (typeof flag === "boolean") {
      return { key, state: flag ? "pass" : "fail" };
    }
    // 证据缺失时仅接受「服务端快照必然蕴含」的回退，不做越级推断
    if (impliedBySnapshot) return { key, state: "pass" };
    return { key, state: "unknown" };
  }

  const verified = lastCheck.verified_repository_count;
  const declared = lastCheck.declared_repository_count;
  let repository: EnvHealthRow;
  if (typeof declared === "number" && declared > 0) {
    repository = {
      key: "repository",
      state: verified === declared ? "pass" : "warning",
      coverage: {
        verified: typeof verified === "number" ? verified : 0,
        declared
      }
    };
  } else {
    repository = { key: "repository", state: "unknown" };
  }

  const authentication: EnvHealthRow =
    device.readiness === "authenticated" || device.readiness === "ready"
      ? { key: "authentication", state: "pass" }
      : { key: "authentication", state: "unknown" };

  return [
    flagRow("installed", lastCheck.installed, rank >= 1),
    flagRow("connected", lastCheck.connected, rank >= 2),
    flagRow("tool", lastCheck.tool, device.readiness === "ready"),
    repository,
    authentication
  ];
}

/** 详情页横幅（为什么可/不可执行 + 恢复动作）→ i18n key 前缀 */
export function bannerKeyPrefix(device: Device): string {
  if (device.status === "revoked") return "environments.banner.revoked";
  switch (device.readiness) {
    case "ready":
      return "environments.banner.ready";
    case "degraded":
      return "environments.banner.degraded";
    case "failed":
      return "environments.banner.failed";
    case null:
      return "environments.banner.unchecked";
    default:
      return "environments.banner.progress";
  }
}

/** 详情页横幅语义 */
export function bannerVariant(
  device: Device
): "success" | "warning" | "danger" | "info" {
  if (device.status === "revoked") return "danger";
  switch (device.readiness) {
    case "ready":
      return "success";
    case "degraded":
    case "failed":
      return "danger";
    case null:
      return "info";
    default:
      return "warning";
  }
}
