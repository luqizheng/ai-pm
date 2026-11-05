import { getCommandCenter } from "./command-center";
import type {
  ActionItem,
  ActivityItem,
  CommandCenter,
  CommandCenterBlock,
  HumanActionSummary,
  InitializationProgress,
  InitItem,
  InitItemStatus,
  ProjectSummary,
  RiskItem
} from "./command-center";

/**
 * S01-01 · 项目初始化总览（S01 · Project Initialization Overview）。
 *
 * **本模块不新建事实来源**：页面消费的是已上线的
 * `GET /v1/pm/projects/{project_id}/command-center` 聚合读模型
 * （后端 crates/pm-application/src/command_center.rs）。
 *
 * 本模块只做三件事：
 *   1. 把后端的 7 个初始化事项投影成人类阅读的 6 个阶段；
 *   2. 按确定性阶梯派生「当前最重要的一件事」；
 *   3. 把待人工处理 / AI 上下文分门别类，供视图渲染。
 *
 * 设计约束（见 docs/ui/s01/s01-01-project-initialization-overview-ux-v1.md）：
 *   - 六阶段是阅读分组，**不是**持久化状态机、不是门禁序列、不是百分比；
 *   - 展示状态只来自后端派生事项；前端不重算领域业务状态；
 *   - 唯一的前端合成项是「基线确认」——后端当前没有基线实体（G-08），
 *     它由两个**后端已派生**的事实合成，且在 UI 上标注来源为 derived。
 */

/**
 * 阶段展示状态 = 后端七态（S01-01 · G-01 裁决已收编）：
 * `waiting_for_human` 已是 `InitItemStatus` 的正式成员，由后端下发。
 */
export type StageStatus = InitItemStatus;

/** 阶段标识 */
export type StageId =
  | "project_basics"
  | "repository"
  | "cognition"
  | "baseline"
  | "environment"
  | "handover";

/**
 * 阶段权威来源：
 * - command_center —— 后端 `initialization_progress` 事项直接给出；
 * - derived —— 由后端**已派生**的事实在前端合成（当前只有基线确认）。
 */
export type StageSource = "command_center" | "derived";

/** 阶段定义（静态结构，不含状态） */
interface StageDef {
  id: StageId;
  order: number;
  /** i18n key 后缀（相对 s01.initialization-overview 命名空间） */
  titleKey: string;
  /** 对应的后端事项 key */
  itemKeys: string[];
  /** 依赖的阶段（软依赖，仅用于提示） */
  dependsOn: StageId[];
  /** 下钻路由 */
  route?: string;
}

/**
 * 六阶段定义。
 *
 * 与后端 7 个事项的对应关系：
 *   project_basics  ← 项目基本信息
 *   repository       ← 仓库接入 + 代码分析（两个事项合成一个阶段）
 *   cognition        ← 项目认知
 *   baseline         ← （后端暂无；由 human_action_summary + 项目认知合成）
 *   environment      ← 开发环境
 *   handover         ← 交接出口（按前 5 个阶段聚合，不直接用「人工处理」事项）
 */
const STAGE_DEFS: StageDef[] = [
  {
    id: "project_basics",
    order: 1,
    titleKey: "stage.projectBasics",
    itemKeys: ["project_basics"],
    dependsOn: []
  },
  {
    id: "repository",
    order: 2,
    titleKey: "stage.repository",
    itemKeys: ["repository_access", "code_analysis"],
    dependsOn: ["project_basics"],
    route: "/project/repository/overview"
  },
  {
    id: "cognition",
    order: 3,
    titleKey: "stage.cognition",
    itemKeys: ["project_cognition"],
    dependsOn: ["repository"],
    route: "/project/cognition"
  },
  {
    id: "baseline",
    order: 4,
    titleKey: "stage.baseline",
    itemKeys: [],
    dependsOn: ["repository", "cognition"],
    route: "/project/baseline"
  },
  {
    id: "environment",
    order: 5,
    titleKey: "stage.environment",
    itemKeys: ["dev_environment"],
    dependsOn: [],
    route: "/project/devenv"
  },
  {
    id: "handover",
    order: 6,
    titleKey: "stage.handover",
    itemKeys: [],
    dependsOn: ["baseline"],
    route: "/dashboard"
  }
];

/** 一个初始化阶段（状态已派生） */
export interface InitStage {
  id: StageId;
  order: number;
  titleKey: string;
  status: StageStatus;
  /** 实况描述（后端 detail 或由结构化数据合成） */
  detail?: string;
  /** 实况描述的 i18n key 后缀（结构化合成时使用，优先于 detail） */
  detailKey?: string;
  /** detailKey 的插值参数 */
  detailParams?: Record<string, string | number>;
  /** 权威来源标识（后端事项状态 / 作业 id / 错误码） */
  sourceState?: string;
  source: StageSource;
  dependsOn: StageId[];
  route?: string;
  /** 回溯用：产出该状态的后端事项 key */
  itemKeys: string[];
}

/** 交接态（设计稿 §3.6.2） */
export type HandoverOutcome = "ready" | "partial" | "blocked" | "undetermined";

/** 责任主体 */
export type ActionOwner = "human" | "ai" | "system";

/** 当前最重要的一件事 */
export interface CurrentAction {
  stageId: StageId;
  titleKey: string;
  status: StageStatus;
  owner: ActionOwner;
  /** 实况（后端原文） */
  detail?: string;
  /** 实况的 i18n key 后缀（结构化合成时使用） */
  detailKey?: string;
  /** detailKey 的插值参数 */
  detailParams?: Record<string, string | number>;
  /** 权威来源 */
  sourceState?: string;
  /** 是否需要用户操作 */
  needsUser: boolean;
  /** 同类其余事项数 */
  remaining: number;
  /** 下钻路由 */
  route?: string;
}

/** AI 上下文条目的事实层级 —— 实心=已成立事实，描边=AI 产出待确认，中性=执行中 */
export type FactLayer = "fact" | "proposal" | "running";

/** AI 上下文分区 */
export type AiContextSectionId =
  | "running"
  | "recent"
  | "proposals"
  | "issues"
  | "next";

/** AI 上下文条目 */
export interface AiContextItem {
  id: string;
  section: AiContextSectionId;
  /** 条目标题（后端数据原文） */
  title: string;
  /** 标题的 i18n key 后缀（阶段派生条目用；优先于 title） */
  titleKey?: string;
  /** 副信息 */
  meta?: string;
  /** 状态标识（后端原始状态，不做翻译） */
  state?: string;
  layer: FactLayer;
}

/** 待人工处理条目（后端 human_action_summary 原样 + 展示补充） */
export interface HumanAction extends ActionItem {
  /** 紧急度轨（design/warning）；仅影响左侧 2px 轨，不影响类型 chip */
  urgency: "none" | "warning" | "danger";
}

/** 初始化总览读模型（视图消费的全部数据） */
export interface InitializationOverview {
  projectId: string;
  /** 各区块独立可用性——不可用不等于 0/空 */
  blocks: CommandCenter["blocks"];
  projectSummary?: ProjectSummary;
  stages: InitStage[];
  currentAction: CurrentAction;
  humanActions: HumanAction[];
  aiContext: AiContextItem[];
  handoverOutcome: HandoverOutcome;
  /** 后端派生的完成判定（当前恒为 not_determinable） */
  completion: string;
}

/* -------------------------------------------------------------------------- */
/* 状态聚合                                                                    */
/* -------------------------------------------------------------------------- */

/**
 * 阶段状态聚合规则（设计稿 §3.2.5）。
 * 顺序敏感：阻塞 > 等待人类 > 进行中 > 尚无法判断 > 未开始 > 完成/不适用。
 */
export function aggregateStatus(statuses: StageStatus[]): StageStatus {
  if (statuses.length === 0) return "undetermined";
  const set = new Set(statuses);
  if (set.has("blocked")) return "blocked";
  if (set.has("waiting_for_human")) return "waiting_for_human";
  if (set.has("in_progress")) return "in_progress";
  if (set.has("undetermined")) return "undetermined";
  if (set.has("not_started")) return "not_started";
  // 全部为 completed / not_applicable：有完成项即视为完成
  if (statuses.some(status => status === "completed")) return "completed";
  return "not_applicable";
}

/* -------------------------------------------------------------------------- */
/* 阶段投影                                                                    */
/* -------------------------------------------------------------------------- */

function itemOf(
  progress: InitializationProgress,
  key: string
): InitItem | undefined {
  return progress.items.find(item => item.key === key);
}

function statusOf(
  progress: InitializationProgress,
  key: string
): StageStatus | undefined {
  return itemOf(progress, key)?.status;
}

/** 待确认类人工事项（finding / 提案）——基线确认阶段的合成输入之一 */
function pendingConfirmations(actions: HumanActionSummary): ActionItem[] {
  return actions.items.filter(
    item => item.domain === "discovery" || item.domain === "knowledge"
  );
}

/** 冲突类风险——基线确认阶段的合成输入之二 */
function conflicts(risks: RiskItem[]): RiskItem[] {
  return risks.filter(
    item => item.kind === "conflict" || item.severity === "high"
  );
}

/**
 * 基线确认阶段派生。
 *
 * 后端当前**没有**基线实体（G-08：无 project_baseline 表、无 baseline 端点），
 * 因此这里用两个后端已派生的事实合成，并如实标注 source = "derived"：
 *   - `human_action_summary` 中的待确认 finding / 提案；
 *   - `risk_summary` 中的冲突项；
 *   - 加上「项目认知」事项的状态。
 */
function deriveBaselineStatus(
  progress: InitializationProgress,
  actions: HumanActionSummary,
  risks: RiskItem[]
): { status: StageStatus; detail?: string; sourceState?: string } {
  const cognition = statusOf(progress, "project_cognition");
  const pending = pendingConfirmations(actions);
  const conflict = conflicts(risks);

  if (conflict.length > 0) {
    return {
      status: "blocked",
      detail: "conflict",
      sourceState: conflict[0].source_id
    };
  }
  if (cognition === "not_started" || cognition === undefined) {
    return { status: "not_started" };
  }
  if (cognition === "blocked") {
    return { status: "blocked" };
  }
  if (cognition === "in_progress") {
    return { status: "not_started", detail: "awaiting_cognition" };
  }
  if (pending.length > 0) {
    return {
      status: "waiting_for_human",
      detail: "pending_confirmations",
      sourceState: String(pending.length)
    };
  }
  if (cognition === "completed") {
    return { status: "completed" };
  }
  return { status: "undetermined" };
}

/**
 * 阶段投影：后端 7 个事项 → 6 个阶段。
 *
 * 阶段 detail 优先用结构化数据合成（走 i18n 模板），拿不到才回落后端 detail。
 */
export function projectStages(
  progress: InitializationProgress,
  actions: HumanActionSummary,
  risks: RiskItem[],
  summary?: ProjectSummary
): InitStage[] {
  return STAGE_DEFS.map(def => {
    const items = def.itemKeys
      .map(key => itemOf(progress, key))
      .filter((item): item is InitItem => Boolean(item));

    let status: StageStatus;
    let detail: string | undefined;
    let detailKey: string | undefined;
    let detailParams: Record<string, string | number> | undefined;
    let sourceState: string | undefined;
    let source: StageSource = "command_center";

    if (def.id === "baseline") {
      const derived = deriveBaselineStatus(progress, actions, risks);
      status = derived.status;
      detail = derived.detail;
      sourceState = derived.sourceState;
      source = "derived";
    } else if (def.id === "handover") {
      const prior = STAGE_DEFS.filter(item => item.id !== "handover").map(
        item => {
          if (item.id === "baseline") {
            return deriveBaselineStatus(progress, actions, risks).status;
          }
          return aggregateStatus(
            item.itemKeys
              .map(key => statusOf(progress, key))
              .filter((value): value is StageStatus => Boolean(value))
          );
        }
      );
      status = aggregateStatus(prior);
      sourceState = progress.completion;
      source = "derived";
    } else {
      status = aggregateStatus(items.map(item => item.status));
      detail = items.find(item => item.detail)?.detail;
      sourceState = items.find(item => item.source_state)?.source_state;
    }

    // 仓库阶段优先用结构化计数（后端 summary 提供了总数与已验证数）：
    // 后端 detail 是中文串，走 i18n 模板才可随语言切换。
    if (def.id === "repository" && summary) {
      detail = undefined;
      detailKey = "stage.repository.detail";
      detailParams = {
        total: summary.repository_count,
        verified: summary.verified_repository_count
      };
      sourceState =
        sourceState ??
        `${summary.repository_count}/${summary.verified_repository_count}`;
    }

    return {
      id: def.id,
      order: def.order,
      titleKey: def.titleKey,
      status,
      detail,
      detailKey,
      detailParams,
      sourceState,
      source,
      dependsOn: def.dependsOn,
      route: def.route,
      itemKeys: def.itemKeys
    };
  });
}

/* -------------------------------------------------------------------------- */
/* 当前最重要的一件事                                                          */
/* -------------------------------------------------------------------------- */

/** 责任主体判定 */
function ownerOf(
  stage: InitStage,
  progress: InitializationProgress,
  actions: HumanActionSummary
): ActionOwner {
  if (stage.id === "baseline") return "human";
  if (stage.id === "repository") {
    return statusOf(progress, "code_analysis") === "in_progress"
      ? "ai"
      : "human";
  }
  if (stage.id === "cognition") return "ai";
  if (stage.id === "environment") {
    return statusOf(progress, "dev_environment") === "in_progress"
      ? "system"
      : "human";
  }
  if (stage.id === "handover") return "system";
  if (actions.total > 0) return "human";
  return "system";
}

/**
 * Current Action 决策阶梯（设计稿 §5.2）。
 * 阻塞 > 等待人类 > AI/系统执行中 > 可启动的下一步 > 无待办。
 */
export function deriveCurrentAction(
  stages: InitStage[],
  progress: InitializationProgress,
  actions: HumanActionSummary
): CurrentAction {
  const pick = (
    predicate: (stage: InitStage) => boolean,
    fallback: InitStage
  ): InitStage => stages.find(predicate) ?? fallback;

  const handover = stages.find(stage => stage.id === "handover")!;

  const blocked = pick(stage => stage.status === "blocked", handover);
  if (blocked.status === "blocked") {
    return {
      stageId: blocked.id,
      titleKey: blocked.titleKey,
      status: blocked.status,
      owner: ownerOf(blocked, progress, actions),
      detail: blocked.detail,
      detailKey: blocked.detailKey,
      detailParams: blocked.detailParams,
      sourceState: blocked.sourceState,
      needsUser: true,
      remaining: stages.filter(stage => stage.status === "blocked").length - 1,
      route: blocked.route
    };
  }

  const waiting = pick(stage => stage.status === "waiting_for_human", handover);
  if (waiting.status === "waiting_for_human") {
    return {
      stageId: waiting.id,
      titleKey: waiting.titleKey,
      status: waiting.status,
      owner: "human",
      detail: waiting.detail,
      detailKey: waiting.detailKey,
      detailParams: waiting.detailParams,
      sourceState: waiting.sourceState,
      needsUser: true,
      remaining:
        stages.filter(stage => stage.status === "waiting_for_human").length - 1,
      route: waiting.route
    };
  }

  const running = pick(
    stage => stage.status === "in_progress" && stage.id !== "handover",
    handover
  );
  if (running.status === "in_progress") {
    return {
      stageId: running.id,
      titleKey: running.titleKey,
      status: running.status,
      owner: ownerOf(running, progress, actions),
      detail: running.detail,
      detailKey: running.detailKey,
      detailParams: running.detailParams,
      sourceState: running.sourceState,
      needsUser: false,
      remaining:
        stages.filter(stage => stage.status === "in_progress").length - 1,
      route: running.route
    };
  }

  const next = pick(
    stage => stage.status === "not_started" && stage.id !== "handover",
    handover
  );
  if (next.status === "not_started") {
    return {
      stageId: next.id,
      titleKey: next.titleKey,
      status: next.status,
      owner: ownerOf(next, progress, actions),
      detail: next.detail,
      detailKey: next.detailKey,
      detailParams: next.detailParams,
      sourceState: next.sourceState,
      needsUser: true,
      remaining:
        stages.filter(stage => stage.status === "not_started").length - 1,
      route: next.route
    };
  }

  return {
    stageId: "handover",
    titleKey: "stage.handover",
    status: handover.status,
    owner: "system",
    detail: handover.sourceState,
    needsUser: false,
    remaining: 0,
    route: handover.route
  };
}

/* -------------------------------------------------------------------------- */
/* 待人工处理 / AI 上下文                                                      */
/* -------------------------------------------------------------------------- */

const URGENT_KINDS = new Set([
  "finding_revision_requested",
  "delivery_pending"
]);

/** 待人工处理条目：类型用中性 chip，颜色只承载紧急度（左侧 2px 轨） */
export function projectHumanActions(
  actions: HumanActionSummary
): HumanAction[] {
  return actions.items.map(item => ({
    ...item,
    urgency: URGENT_KINDS.has(item.kind)
      ? "danger"
      : item.kind === "finding_pending" || item.kind === "proposal_pending"
        ? "warning"
        : "none"
  }));
}

/**
 * AI 上下文分区构建：执行中 / 最近完成 / 待确认提案 / 当前问题 / 下一步。
 *
 * 数据来源纪律（重要）：
 *   - 后端活动流（recent_activity）**只记录已发生的动作**（analysis_succeeded、
 *     repository_verified、run_finished …），没有「开始」事件。
 *     因此「AI 正在做什么」必须来自**阶段派生**（作业状态 source_state），
 *     不能从活动流推断——否则会把「已完成」显示成「执行中」。
 *   - 后端 activity_type 是 snake_case（analysis_succeeded / analysis_failed …），
 *     与前端既有 pm.commandCenter.activity.*（camelCase）词表不一致，此处按
 *     后端口径处理，词表差异登记为待修项。
 */
export function projectAiContext(
  stages: InitStage[],
  activity: ActivityItem[],
  risks: RiskItem[],
  actions: HumanActionSummary
): AiContextItem[] {
  const items: AiContextItem[] = [];

  // ① AI 执行中：由阶段派生（作业状态），不取自活动流
  for (const stage of stages) {
    if (stage.id === "handover") continue;
    if (stage.status !== "in_progress") continue;
    items.push({
      id: `${stage.id}-running`,
      section: "running",
      title: stage.titleKey,
      titleKey: stage.titleKey,
      meta: stage.sourceState,
      state: stage.status,
      layer: "running"
    });
  }

  // ② 最近完成：Agent 完成的动作（事实）
  for (const event of activity) {
    if (event.actor.kind === "human") continue;
    items.push({
      id: event.id,
      section: "recent",
      title: event.summary,
      meta: event.source_id,
      state: event.activity_type,
      layer: "fact"
    });
  }

  // ③ 已形成的提案（AI 产出，待确认）：来自待人工处理中的 finding / proposal
  for (const action of actions.items) {
    if (action.domain !== "discovery" && action.domain !== "knowledge")
      continue;
    items.push({
      id: action.id,
      section: "proposals",
      title: action.title,
      meta: action.detail,
      state: action.source_state,
      layer: "proposal"
    });
  }

  // ④ 当前发现的问题（事实）
  for (const risk of risks) {
    items.push({
      id: risk.source_id,
      section: "issues",
      title: risk.title,
      meta: risk.suggestion,
      state: risk.severity,
      layer: "fact"
    });
  }

  // ⑤ 下一步：尚未完成且不在执行中的阶段（由阶段状态派生，不是 Agent 自由文本）
  for (const stage of stages) {
    if (stage.id === "handover") continue;
    if (stage.status === "completed" || stage.status === "not_applicable") {
      continue;
    }
    if (stage.status === "in_progress") continue;
    items.push({
      id: `${stage.id}-next`,
      section: "next",
      title: stage.titleKey,
      titleKey: stage.titleKey,
      meta: stage.sourceState,
      state: stage.status,
      layer: "proposal"
    });
  }

  return items;
}

/* -------------------------------------------------------------------------- */
/* 读取入口                                                                    */
/* -------------------------------------------------------------------------- */

/** 交接态判定（设计稿 §3.6.2） */
export function deriveHandoverOutcome(
  stages: InitStage[],
  completion: string
): HandoverOutcome {
  if (stages.some(stage => stage.status === "blocked")) return "blocked";
  if (completion === "not_determinable") return "undetermined";
  const pending = stages.some(
    stage => stage.id !== "handover" && stage.status !== "completed"
  );
  return pending ? "partial" : "ready";
}

/**
 * 拉取并投影初始化总览。
 *
 * 后端端点：`GET /v1/pm/projects/{project_id}/command-center`
 */
export async function loadInitializationOverview(
  projectId: string
): Promise<InitializationOverview> {
  const center: CommandCenter = await getCommandCenter(projectId);

  const progressBlock: CommandCenterBlock<InitializationProgress> =
    center.blocks.initialization_progress;
  const actionBlock: CommandCenterBlock<HumanActionSummary> =
    center.blocks.human_action_summary;
  const riskBlock: CommandCenterBlock<{ items: RiskItem[]; total: number }> =
    center.blocks.risk_summary;
  const activityBlock: CommandCenterBlock<{
    items: ActivityItem[];
    total: number;
  }> = center.blocks.recent_activity;
  const summaryBlock: CommandCenterBlock<ProjectSummary> =
    center.blocks.project_summary;

  const progress = progressBlock.data;
  const actions = actionBlock.data ?? { total: 0, items: [] };
  const risks = riskBlock.data?.items ?? [];
  const activity = activityBlock.data?.items ?? [];
  const summary = summaryBlock.data;

  const stages = progress
    ? projectStages(progress, actions, risks, summary)
    : STAGE_DEFS.map(def => ({
        id: def.id,
        order: def.order,
        titleKey: def.titleKey,
        status: "undetermined" as StageStatus,
        source: "derived" as StageSource,
        dependsOn: def.dependsOn,
        route: def.route,
        itemKeys: def.itemKeys
      }));

  const currentAction = progress
    ? deriveCurrentAction(stages, progress, actions)
    : {
        stageId: "handover" as StageId,
        titleKey: "stage.handover",
        status: "undetermined" as StageStatus,
        owner: "system" as ActionOwner,
        needsUser: false,
        remaining: 0
      };

  return {
    projectId: center.project_id,
    blocks: center.blocks,
    projectSummary: summary,
    stages,
    currentAction,
    humanActions: projectHumanActions(actions),
    aiContext: projectAiContext(stages, activity, risks, actions),
    handoverOutcome: deriveHandoverOutcome(
      stages,
      progress?.completion ?? "not_determinable"
    ),
    completion: progress?.completion ?? "not_determinable"
  };
}
