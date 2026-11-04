/**
 * S01 初始化域 mock —— 仅供 build/mock-plugin.ts（node 侧）使用。
 *
 * 覆盖 S01-01~04 的前端联调端点，字段与
 * `docs/s01-backend-api-requirements-v1.md` 完全一致。
 *
 * 与 fixtures.ts 一样：**不得 import 浏览器 API / Vue**。
 */

export type MockResponse = {
  status: number;
  body: unknown;
};

function iso(daysAgo: number, hour = 10): string {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  d.setHours(hour, 30, 0, 0);
  return d.toISOString();
}

/** 演示项目 ID（与 fixtures.ts 的 projects 首行一致）。 */
export const DEMO_PROJECT_ID = "prj-shield";

/* ───────────────────────── S01-03 仓库（v2 语义） ───────────────────────── */

export const repositoriesV2 = [
  {
    id: "repo-frontend",
    project_id: DEMO_PROJECT_ID,
    name: "前端主仓库",
    url: "git@gitlab.local:team/shield-web.git",
    provider: "gitlab",
    repo_owner: "team",
    role: "primary",
    default_branch: "main",
    selected_branch: "main",
    selected_commit: "a1b2c3d4e5f6",
    connection_status: "connected",
    verification_status: "passed",
    last_verified_at: iso(1, 9),
    last_scanned_at: iso(2, 15),
    head_commit_sha: "a1b2c3d4e5f6",
    description: "客服工作台前端",
    revision: 7,
    created_at: iso(60),
    updated_at: iso(1, 9)
  },
  {
    id: "repo-backend",
    project_id: DEMO_PROJECT_ID,
    name: "后端服务仓库",
    url: "git@gitlab.local:team/shield-svc.git",
    provider: "gitlab",
    repo_owner: "team",
    role: "supporting",
    default_branch: "main",
    selected_branch: "main",
    // HEAD 已前进 —— 前端据此提示「仓库已有新提交」，但不自动更新采用版本
    selected_commit: "f6e5d4c3b2a1",
    connection_status: "connected",
    verification_status: "warning",
    last_verified_at: iso(1, 9),
    last_scanned_at: iso(3, 11),
    head_commit_sha: "0a1b2c3d4e5f",
    description: "工单与路由服务",
    revision: 12,
    created_at: iso(58),
    updated_at: iso(1, 9)
  },
  {
    id: "repo-infra",
    project_id: DEMO_PROJECT_ID,
    name: "基础设施仓库",
    url: "git@gitlab.local:team/shield-infra.git",
    provider: "gitlab",
    repo_owner: "team",
    role: "infrastructure",
    default_branch: "master",
    selected_branch: "master",
    selected_commit: "9988776655",
    connection_status: "connected",
    verification_status: "passed",
    last_verified_at: iso(4, 16),
    last_scanned_at: null,
    head_commit_sha: "9988776655",
    description: "部署与环境配置",
    revision: 3,
    created_at: iso(40),
    updated_at: iso(4, 16)
  },
  {
    id: "repo-doc",
    project_id: DEMO_PROJECT_ID,
    name: "文档仓库",
    url: "git@gitlab.local:team/shield-docs.git",
    provider: "gitlab",
    repo_owner: "team",
    role: "supporting",
    default_branch: "main",
    selected_branch: null,
    selected_commit: null,
    connection_status: "disconnected",
    verification_status: "pending",
    last_verified_at: null,
    last_scanned_at: null,
    head_commit_sha: null,
    description: "已断开 —— 历史记录仍保留",
    revision: 5,
    created_at: iso(35),
    updated_at: iso(9, 14)
  }
];

export const gitProviders = [
  {
    provider: "gitlab",
    scopes: ["repo:read"],
    status: "active",
    expires_at: iso(-30),
    authorized_at: iso(45)
  },
  {
    provider: "github",
    scopes: [],
    status: "expired",
    expires_at: iso(3),
    authorized_at: iso(90)
  }
];

/* ───────────────────────── S01-01 基线 ───────────────────────── */

export const baselineProposals = [
  {
    id: "bp-3",
    project_id: DEMO_PROJECT_ID,
    proposal_version: 3,
    status: "pending_review",
    generation_reason:
      "后端服务仓库 HEAD 已前进（f6e5d4c → 0a1b2c3），且新增了工单导出能力，重新生成本项目理解。",
    analysis_scope: [
      "project_identity",
      "repository_understanding",
      "architecture_understanding"
    ],
    sources: [
      {
        source_type: "git_commit",
        label: "后端服务仓库 · main @ 0a1b2c3",
        detail: "新增工单导出模块与异步导出任务",
        locator: "crates/svc/src/export"
      },
      {
        source_type: "code_fact",
        label: "客服工作台前端 · src/pages",
        detail: "前端路由含 8 个业务页面",
        locator: "src/pages"
      },
      {
        source_type: "doc_fact",
        label: "项目需求文档 v3",
        detail: "2026 Q4 需求：批量导出",
        locator: "docs/requirements/2026Q4.md#L42"
      },
      {
        source_type: "ai_inference",
        label: "推断：项目以工单编排为核心",
        detail: "基于模块依赖图推断，未经人类确认",
        locator: null
      }
    ],
    repositories: [
      {
        repository_id: "repo-frontend",
        repository_name: "前端主仓库",
        repository_url: "git@gitlab.local:team/shield-web.git",
        branch_name: "main",
        commit_sha: "a1b2c3d4e5f6"
      },
      {
        repository_id: "repo-backend",
        repository_name: "后端服务仓库",
        repository_url: "git@gitlab.local:team/shield-svc.git",
        branch_name: "main",
        commit_sha: "0a1b2c3d4e5f"
      }
    ],
    content: [
      {
        key: "identity",
        title: "项目身份",
        body: "星河客服中台 —— 面向星河集团的多渠道客服与工单系统。"
      },
      {
        key: "modules",
        title: "模块职责",
        body: "工作台前端负责坐席交互；服务仓库承载工单生命周期、路由与导出。"
      },
      {
        key: "stack",
        title: "技术栈",
        body: "前端 Vue 3 + TypeScript；服务端 Rust + PostgreSQL。"
      },
      {
        key: "constraints",
        title: "架构约束",
        body: "所有写操作必须经由工单状态机，不允许直接改状态字段。"
      }
    ],
    diff: [
      {
        diff_type: "added",
        section: "模块职责",
        field: null,
        before: null,
        after: "新增「工单导出」模块（异步任务 + 审计留痕）",
        reason: "后端服务仓库 main 分支新增 export 模块",
        locator: "crates/svc/src/export"
      },
      {
        diff_type: "modified",
        section: "技术栈",
        field: "服务端框架",
        before: "Rust + Axum",
        after: "Rust + Axum + Tokio 多 worker",
        reason: "导出任务需要并发调度",
        locator: null
      },
      {
        diff_type: "removed",
        section: "模块职责",
        field: null,
        before: "「手工导出」（CSV 直出）",
        after: null,
        reason: "已被异步导出任务取代",
        locator: null
      },
      {
        diff_type: "modified",
        section: "架构约束",
        field: null,
        before: "写操作须经工单状态机",
        after: "写操作须经工单状态机；导出任务不得直接写业务表",
        reason: "异步导出的写边界澄清",
        locator: null
      },
      {
        diff_type: "unchanged",
        section: "项目身份",
        field: null,
        before: "星河客服中台 —— 面向星河集团的多渠道客服与工单系统。",
        after: "星河客服中台 —— 面向星河集团的多渠道客服与工单系统。",
        reason: null,
        locator: null
      },
      {
        diff_type: "unchanged",
        section: "业务领域",
        field: null,
        before: "工单 / 坐席 / 渠道 / 路由",
        after: "工单 / 坐席 / 渠道 / 路由",
        reason: null,
        locator: null
      }
    ],
    risks: [
      {
        risk_id: "risk-1",
        risk_level: "P0",
        risk_title: "异步导出绕过工单状态机",
        risk_description:
          "导出任务直接读取业务表，绕过了既有的写操作约束；若导出逻辑被误复用，可能产生脏数据。",
        ai_status: "resolved",
        human_decision: null,
        evidence_ref: "crates/svc/src/export/reader.rs#L42",
        decided_by: null,
        decided_at: null,
        decision_note: null
      },
      {
        risk_id: "risk-2",
        risk_level: "P1",
        risk_title: "导出任务的权限边界未定义",
        risk_description:
          "谁可以发起导出、导出结果对谁可见，提案未给出明确规则。",
        ai_status: null,
        human_decision: null,
        evidence_ref: null,
        decided_by: null,
        decided_at: null,
        decision_note: null
      },
      {
        risk_id: "risk-3",
        risk_level: "P2",
        risk_title: "术语「导出」与「报表」混用",
        risk_description: "文档与代码里两个词指向同一能力，建议后续统一术语。",
        ai_status: null,
        human_decision: null,
        evidence_ref: null,
        decided_by: null,
        decided_at: null,
        decision_note: null
      }
    ],
    repository_baselines: [
      {
        repository_id: "repo-frontend",
        repository_name: "前端主仓库",
        repository_url: "git@gitlab.local:team/shield-web.git",
        branch_name: "main",
        commit_sha: "a1b2c3d4e5f6"
      },
      {
        repository_id: "repo-backend",
        repository_name: "后端服务仓库",
        repository_url: "git@gitlab.local:team/shield-svc.git",
        branch_name: "main",
        commit_sha: "0a1b2c3d4e5f"
      }
    ],
    created_at: iso(0, 9),
    updated_at: iso(0, 10),
    confirmed_at: null,
    rejection_reason: null,
    failure_reason: null
  },
  {
    id: "bp-2",
    project_id: DEMO_PROJECT_ID,
    proposal_version: 2,
    status: "confirmed",
    generation_reason: "首次建立基线。",
    analysis_scope: ["project_identity", "repository_understanding"],
    sources: [
      {
        source_type: "doc_fact",
        label: "项目立项文档",
        detail: "2026 Q3 立项",
        locator: "docs/plan/2026Q3.md"
      }
    ],
    repositories: [],
    content: [
      {
        key: "identity",
        title: "项目身份",
        body: "星河客服中台 —— 面向星河集团的多渠道客服与工单系统。"
      }
    ],
    diff: [],
    risks: [],
    repository_baselines: [],
    created_at: iso(60),
    updated_at: iso(60),
    confirmed_at: iso(58),
    rejection_reason: null,
    failure_reason: null
  }
];

export const baselines = [
  {
    id: "bl-2",
    project_id: DEMO_PROJECT_ID,
    version: 2,
    content: [
      {
        key: "identity",
        title: "项目身份",
        body: "星河客服中台 —— 面向星河集团的多渠道客服与工单系统。"
      },
      {
        key: "modules",
        title: "模块职责",
        body: "工作台前端负责坐席交互；服务仓库承载工单生命周期与路由。"
      }
    ],
    repositories: [
      {
        repository_id: "repo-frontend",
        repository_name: "前端主仓库",
        repository_url: "git@gitlab.local:team/shield-web.git",
        branch_name: "main",
        commit_sha: "a1b2c3d4e5f6"
      }
    ],
    proposal_id: "bp-2",
    proposal_version: 2,
    confirmed_by: "prc-zhang",
    confirmed_by_name: "张伟",
    confirmed_at: iso(58),
    superseded_at: null,
    created_at: iso(58)
  },
  {
    id: "bl-1",
    project_id: DEMO_PROJECT_ID,
    version: 1,
    content: [
      {
        key: "identity",
        title: "项目身份",
        body: "星河客服中台。"
      }
    ],
    repositories: [],
    proposal_id: null,
    proposal_version: null,
    confirmed_by: "prc-zhang",
    confirmed_by_name: "张伟",
    confirmed_at: iso(60),
    superseded_at: iso(58),
    created_at: iso(60)
  }
];

/* ───────────────────────── S01-04 AI-PM 认知 ───────────────────────── */

export const cognitionRuns = [
  {
    id: "run-2",
    project_id: DEMO_PROJECT_ID,
    status: "completed",
    scope: [
      "project_identity",
      "repository_understanding",
      "architecture_understanding",
      "development_convention"
    ],
    repository_baselines: [
      {
        repository_id: "repo-frontend",
        repository_name: "前端主仓库",
        repository_url: "git@gitlab.local:team/shield-web.git",
        branch_name: "main",
        commit_sha: "a1b2c3d4e5f6"
      }
    ],
    inputs: ["项目需求文档 v3", "README.md", "package.json", "Cargo.toml"],
    capability_version: "cognition-1.2",
    item_count: 5,
    confirmed_count: 2,
    error_message: null,
    started_at: iso(1, 9),
    completed_at: iso(1, 10),
    created_at: iso(1, 9)
  },
  {
    id: "run-1",
    project_id: DEMO_PROJECT_ID,
    status: "partially_completed",
    scope: ["project_identity", "domain_understanding"],
    repository_baselines: [],
    inputs: ["项目立项文档"],
    capability_version: "cognition-1.1",
    item_count: 3,
    confirmed_count: 3,
    error_message: "domain_understanding 分析超时，部分领域未完成",
    started_at: iso(12, 14),
    completed_at: iso(12, 15),
    created_at: iso(12, 14)
  }
];

export const cognitionItems = [
  {
    id: "ci-1",
    run_id: "run-2",
    domain: "project_identity",
    content:
      "项目目标是让星河集团的客服团队在单一工作台处理多渠道工单，核心指标是首次响应时长。",
    evidence: [
      {
        source_type: "doc_fact",
        label: "项目需求文档 v3 §1",
        detail: "明确写明核心指标为首响时长",
        locator: "docs/requirements/2026Q4.md#L8"
      }
    ],
    evidence_type: "doc_fact",
    confidence: 0.93,
    has_conflict: false,
    status: "proposed",
    change_type: "add",
    related_knowledge_id: null,
    ai_rationale:
      "从需求文档首段抽取目标句，并识别到可量化的指标表述；未发现代码侧反证。",
    decided_by: null,
    decided_at: null,
    decision_note: null,
    created_at: iso(1, 9)
  },
  {
    id: "ci-2",
    run_id: "run-2",
    domain: "architecture_understanding",
    content: "工单写操作全部经由工单状态机，仓储层不暴露直接状态写入接口。",
    evidence: [
      {
        source_type: "code_fact",
        label: "services/svc/src/ticket/state.rs",
        detail: "唯一状态变更入口 apply_transition",
        locator: "crates/svc/src/ticket/state.rs#L18"
      },
      {
        source_type: "code_fact",
        label: "仓储层接口",
        detail: "无 set_status 方法",
        locator: "crates/svc/src/ticket/repo.rs"
      }
    ],
    evidence_type: "code_fact",
    confidence: 0.97,
    has_conflict: false,
    status: "proposed",
    change_type: "add",
    related_knowledge_id: null,
    ai_rationale: "两处代码事实互证，置信度高。",
    decided_by: null,
    decided_at: null,
    decision_note: null,
    created_at: iso(1, 9)
  },
  {
    id: "ci-3",
    run_id: "run-2",
    domain: "development_convention",
    content: "服务端提交信息使用 Conventional Commits（feat/fix/chore 前缀）。",
    evidence: [
      {
        source_type: "code_fact",
        label: "近 30 次提交标题",
        detail: "27/30 符合 Conventional Commits",
        locator: null
      }
    ],
    evidence_type: "code_fact",
    confidence: 0.88,
    has_conflict: false,
    status: "proposed",
    change_type: "add",
    related_knowledge_id: null,
    ai_rationale: "统计提交标题模式得出。",
    decided_by: null,
    decided_at: null,
    decision_note: null,
    created_at: iso(1, 9)
  },
  {
    id: "ci-4",
    run_id: "run-2",
    domain: "project_identity",
    content: "项目的核心业务概念是「工单」，而非「会话」或「客户」。",
    evidence: [
      {
        source_type: "ai_inference",
        label: "推断：领域模型以工单为中心",
        detail: "基于实体命名与引用频次推断，未见明确定义文档",
        locator: null
      }
    ],
    evidence_type: "ai_inference",
    confidence: 0.54,
    has_conflict: true,
    status: "proposed",
    change_type: "add",
    related_knowledge_id: null,
    ai_rationale:
      "需求文档同时出现「会话」「客户」，与代码里的「工单」存在术语冲突，需人类裁决。",
    decided_by: null,
    decided_at: null,
    decision_note: null,
    created_at: iso(1, 9)
  },
  {
    id: "ci-5",
    run_id: "run-2",
    domain: "repository_understanding",
    content: "前端使用 Vue 3 + TypeScript，构建工具为 Vite。",
    evidence: [
      {
        source_type: "code_fact",
        label: "package.json",
        detail: "vue 3.5 / vite 8 / typescript 6",
        locator: "package.json"
      }
    ],
    evidence_type: "code_fact",
    confidence: 0.99,
    has_conflict: false,
    status: "confirmed",
    change_type: "add",
    related_knowledge_id: "kn-1",
    ai_rationale: "直接读 package.json。",
    decided_by: "prc-zhang",
    decided_at: iso(1, 11),
    decision_note: "与仓库一致，直接确认。",
    created_at: iso(1, 9)
  }
];

export const projectKnowledge = [
  {
    id: "kn-1",
    project_id: DEMO_PROJECT_ID,
    category: "technology",
    content: "前端使用 Vue 3 + TypeScript，构建工具为 Vite。",
    source_references: [
      {
        source_type: "code_fact",
        label: "package.json",
        detail: null,
        locator: "package.json"
      }
    ],
    confirmed_by: "prc-zhang",
    confirmed_by_name: "张伟",
    confirmed_at: iso(1, 11),
    version: 1,
    status: "active",
    supersedes: null,
    is_core: true,
    created_at: iso(1, 11)
  },
  {
    id: "kn-2",
    project_id: DEMO_PROJECT_ID,
    category: "architecture",
    content: "工单写操作全部经由工单状态机。",
    source_references: [
      {
        source_type: "code_fact",
        label: "state.rs",
        detail: null,
        locator: "crates/svc/src/ticket/state.rs#L18"
      }
    ],
    confirmed_by: "prc-zhang",
    confirmed_by_name: "张伟",
    confirmed_at: iso(20, 16),
    version: 2,
    status: "active",
    supersedes: "kn-2-v1",
    is_core: true,
    created_at: iso(20, 16)
  },
  {
    id: "kn-3",
    project_id: DEMO_PROJECT_ID,
    category: "business",
    content: "项目面向星河集团客服团队，覆盖电话、在线、邮件三类渠道。",
    source_references: [
      {
        source_type: "doc_fact",
        label: "项目立项文档",
        detail: null,
        locator: "docs/plan/2026Q3.md"
      }
    ],
    confirmed_by: "prc-li",
    confirmed_by_name: "李娜",
    confirmed_at: iso(40, 11),
    version: 1,
    status: "active",
    supersedes: null,
    is_core: false,
    created_at: iso(40, 11)
  }
];

export const agentContext = {
  base_context: { knowledge: projectKnowledge.filter(k => k.is_core) },
  task_context: { knowledge: [] },
  repository_context: {
    repository_id: "repo-frontend",
    branch: "main",
    commit: "a1b2c3d4e5f6",
    knowledge: [projectKnowledge[1]]
  },
  execution_context: { run_id: null, task_id: null, state: "idle" },
  generated_at: iso(0, 10)
};

/* ───────────────────────── 解析器 ───────────────────────── */

/** 可变状态：mock 内对提案/风险的裁决与认知处理会改写这些数组。 */
const state = {
  proposals: structuredClone(baselineProposals),
  baselines: structuredClone(baselines),
  items: structuredClone(cognitionItems),
  knowledge: structuredClone(projectKnowledge),
  repositories: structuredClone(repositoriesV2),
  runs: structuredClone(cognitionRuns)
};

function paginate<T>(items: T[], query: URLSearchParams) {
  const page = Number(query.get("page") ?? "1");
  const size = Number(query.get("size") ?? "20");
  const start = (page - 1) * size;
  return {
    items: items.slice(start, start + size),
    total: items.length,
    page,
    size
  };
}

/** 名称 → Project Key 建议（与后端约定的规则一致：字母数字大写，截断 32）。 */
function suggestCode(name: string): {
  code: string;
  available: boolean;
  conflict_code: string | null;
} {
  const letters = name.replace(/[^0-9A-Za-z]/g, "").toUpperCase();
  const code = (letters || "PROJECT").slice(0, 32);
  const taken = ["SHIELD", "ATLAS", "NOVA"].includes(code);
  return {
    code,
    available: !taken,
    conflict_code: taken ? code : null
  };
}

export function resolveInitializationMock(
  method: string,
  pathname: string,
  query: URLSearchParams,
  body?: unknown
): MockResponse | undefined {
  const payload = (body ?? {}) as Record<string, unknown>;

  /* ---------- GET ---------- */
  if (method === "GET") {
    // S01-02 Project Key 建议
    if (pathname === "/v1/pm/projects/suggest-code") {
      return {
        status: 200,
        body: suggestCode(String(query.get("name") ?? ""))
      };
    }

    // 初始化状态
    const init = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/initialization$/
    );
    if (init) {
      return {
        status: 200,
        body: {
          project_id: init[1],
          project_status: "active",
          init_status: "in_progress",
          current_baseline_id: "bl-2",
          baseline_version: 2,
          repository_count: state.repositories.length,
          primary_repository_id: "repo-frontend",
          cognition_run_count: state.runs.length,
          pending_cognition_items: state.items.filter(
            i => i.status === "proposed"
          ).length,
          pending_baseline_proposals: state.proposals.filter(
            p => p.status === "pending_review"
          ).length,
          updated_at: iso(0, 10)
        }
      };
    }

    // 项目背景信息
    const profile = pathname.match(/^\/v1\/pm\/projects\/([^/]+)\/profile$/);
    if (profile) {
      return {
        status: 200,
        body: {
          project_id: profile[1],
          project_type: "web_app",
          business_domain: "客户支持 / 工单系统",
          expected_outcome: "把多渠道工单收敛到统一坐席工作台",
          initial_context: "对接星河集团 CRM，账号体系由集团统一签发。",
          updated_at: iso(2, 14)
        }
      };
    }

    // S01-03 仓库列表（v2）
    const repoList = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/repositories$/
    );
    if (repoList) {
      return {
        status: 200,
        body: {
          items: state.repositories,
          total: state.repositories.length
        }
      };
    }

    // Provider 授权
    const providers = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/git-providers$/
    );
    if (providers) {
      return { status: 200, body: gitProviders };
    }

    // S01-04 认知准备
    const preparation = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/cognition\/preparation$/
    );
    if (preparation) {
      return {
        status: 200,
        body: {
          project_info_complete: true,
          missing_project_fields: [],
          repositories: state.repositories
            .filter(r => r.connection_status === "connected")
            .map(r => ({
              repository_id: r.id,
              name: r.name,
              authorized: true,
              accessible: true,
              collected_documents: 12,
              missing: []
            })),
          collected_count: 34,
          missing_materials: [
            {
              code: "no_primary_doc",
              message: "尚未提供对外接口契约文档",
              blocking: false
            }
          ],
          analysis_scope: [
            "project_identity",
            "repository_understanding",
            "architecture_understanding",
            "development_convention"
          ]
        }
      };
    }

    // 认知 Run 列表
    const runs = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/cognition\/runs$/
    );
    if (runs) {
      return { status: 200, body: paginate(state.runs, query) };
    }

    // 认知条目列表
    const items = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/cognition\/items$/
    );
    if (items) {
      const status = query.get("status");
      const domain = query.get("domain");
      const runId = query.get("run_id");
      let filtered = state.items;
      if (status) filtered = filtered.filter(i => i.status === status);
      if (domain) filtered = filtered.filter(i => i.domain === domain);
      if (runId) filtered = filtered.filter(i => i.run_id === runId);
      return { status: 200, body: paginate(filtered, query) };
    }

    // 正式知识列表
    const knowledge = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/knowledge$/
    );
    if (knowledge) {
      const category = query.get("category");
      const status = query.get("status");
      let filtered = state.knowledge;
      if (category) filtered = filtered.filter(k => k.category === category);
      if (status) filtered = filtered.filter(k => k.status === status);
      return { status: 200, body: paginate(filtered, query) };
    }

    // 知识变更历史
    const knowledgeHistory = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/knowledge\/([^/]+)\/history$/
    );
    if (knowledgeHistory) {
      return {
        status: 200,
        body: { items: state.knowledge, total: state.knowledge.length }
      };
    }

    // Agent Context
    const context = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/agent-context$/
    );
    if (context) {
      return { status: 200, body: agentContext };
    }

    // S01-01 基线总览
    const baselineOverview = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline$/
    );
    if (baselineOverview) {
      const current =
        state.baselines.find(b => b.superseded_at === null) ?? null;
      return {
        status: 200,
        body: {
          current,
          history: state.baselines.filter(b => b.superseded_at !== null),
          pending_proposals: state.proposals.filter(
            p => p.status === "pending_review"
          ).length
        }
      };
    }

    // 基线历史
    const baselineHistory = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/history$/
    );
    if (baselineHistory) {
      return {
        status: 200,
        body: {
          items: state.baselines,
          total: state.baselines.length
        }
      };
    }

    // 基线版本详情
    const baselineDetail = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/history\/([^/]+)$/
    );
    if (baselineDetail) {
      const baseline = state.baselines.find(b => b.id === baselineDetail[2]);
      if (!baseline) return { status: 404, body: { error: "not_found" } };
      return {
        status: 200,
        body: {
          baseline,
          confirmation: {
            id: `cf-${baseline.id}`,
            baseline_id: baseline.id,
            baseline_version: baseline.version,
            proposal_id: baseline.proposal_id ?? "bp-1",
            proposal_version: baseline.proposal_version ?? 1,
            confirmed_by: baseline.confirmed_by ?? "prc-zhang",
            confirmed_by_name: baseline.confirmed_by_name ?? "张伟",
            confirmed_at: baseline.confirmed_at,
            confirmation_note: "首版基线，范围与目标确认无误。",
            repository_baselines: baseline.repositories,
            resulting_baseline_version: baseline.version
          }
        }
      };
    }

    // 提案列表
    const proposalList = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/proposals$/
    );
    if (proposalList) {
      return { status: 200, body: paginate(state.proposals, query) };
    }

    // 提案详情
    const proposalDetail = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/proposals\/([^/]+)$/
    );
    if (proposalDetail) {
      const proposal = state.proposals.find(p => p.id === proposalDetail[2]);
      if (!proposal) return { status: 404, body: { error: "not_found" } };
      return { status: 200, body: proposal };
    }

    // 风险列表
    const riskList = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/proposals\/([^/]+)\/risks$/
    );
    if (riskList) {
      const proposal = state.proposals.find(p => p.id === riskList[2]);
      if (!proposal) return { status: 404, body: { error: "not_found" } };
      return {
        status: 200,
        body: { items: proposal.risks, total: proposal.risks.length }
      };
    }
  }

  /* ---------- POST ---------- */
  if (method === "POST") {
    // S01-02 创建项目
    if (pathname === "/v1/pm/projects") {
      const code = String(payload.code ?? "").trim();
      const name = String(payload.name ?? "").trim();
      if (!code || !name) {
        return {
          status: 400,
          body: { message: "project code and name are required" }
        };
      }
      return {
        status: 201,
        body: {
          id: `prj-mock-${Date.now()}`,
          code,
          name,
          customer_id: payload.customer_id ?? "cus-xinghe",
          owner_principal_id: payload.owner_principal_id ?? "prc-zhang",
          status: "planning",
          revision: 1,
          init_status: "not_started",
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString()
        }
      };
    }

    // 初始化状态更新
    const initUpdate = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/initialization$/
    );
    if (initUpdate) {
      return {
        status: 200,
        body: {
          project_id: initUpdate[1],
          project_status: "active",
          init_status: payload.init_status ?? "in_progress",
          current_baseline_id: "bl-2",
          baseline_version: 2,
          repository_count: state.repositories.length,
          primary_repository_id: "repo-frontend",
          cognition_run_count: state.runs.length,
          pending_cognition_items: state.items.filter(
            i => i.status === "proposed"
          ).length,
          pending_baseline_proposals: state.proposals.filter(
            p => p.status === "pending_review"
          ).length,
          updated_at: new Date().toISOString()
        }
      };
    }

    // S01-03 仓库核验
    const verify = pathname.match(/^\/v1\/pm\/repositories\/([^/]+)\/verify$/);
    if (verify) {
      const repo = state.repositories.find(r => r.id === verify[1]);
      if (!repo) return { status: 404, body: { error: "not_found" } };
      const headChanged =
        repo.selected_commit !== null && repo.head_commit_sha !== null
          ? repo.selected_commit !== repo.head_commit_sha
          : false;
      return {
        status: 200,
        body: {
          repository_id: repo.id,
          status: "verified",
          verification_status: repo.verification_status,
          default_branch: repo.default_branch,
          head_commit_sha: repo.head_commit_sha,
          selected_branch: repo.selected_branch,
          selected_commit: repo.selected_commit,
          head_changed: headChanged,
          checks: [
            { code: "repository_identity", status: "passed", detail: null },
            { code: "access_permission", status: "passed", detail: null },
            { code: "branch_availability", status: "passed", detail: null },
            {
              code: "commit_availability",
              status: headChanged ? "warning" : "passed",
              detail: headChanged ? "分支 HEAD 已前进" : null
            },
            { code: "repository_metadata", status: "passed", detail: null }
          ],
          verified_at: new Date().toISOString(),
          error_code: null,
          error_detail: null
        }
      };
    }

    // 设为主仓库
    const setPrimary = pathname.match(
      /^\/v1\/pm\/repositories\/([^/]+)\/set-primary$/
    );
    if (setPrimary) {
      for (const repo of state.repositories) {
        if (repo.role === "primary") repo.role = "supporting";
      }
      const target = state.repositories.find(r => r.id === setPrimary[1]);
      if (!target) return { status: 404, body: { error: "not_found" } };
      target.role = "primary";
      return { status: 200, body: target };
    }

    // 更新采用版本
    const updateVersion = pathname.match(
      /^\/v1\/pm\/repositories\/([^/]+)\/update-version$/
    );
    if (updateVersion) {
      const target = state.repositories.find(r => r.id === updateVersion[1]);
      if (!target) return { status: 404, body: { error: "not_found" } };
      target.selected_branch = String(payload.selected_branch ?? "");
      target.selected_commit = String(payload.selected_commit ?? "");
      target.updated_at = new Date().toISOString();
      return { status: 200, body: target };
    }

    // 断开 / 恢复
    const disconnect = pathname.match(
      /^\/v1\/pm\/repositories\/([^/]+)\/disconnect$/
    );
    if (disconnect) {
      const target = state.repositories.find(r => r.id === disconnect[1]);
      if (!target) return { status: 404, body: { error: "not_found" } };
      target.connection_status = "disconnected";
      target.verification_status = "pending";
      target.updated_at = new Date().toISOString();
      return { status: 200, body: target };
    }

    const restore = pathname.match(
      /^\/v1\/pm\/repositories\/([^/]+)\/restore$/
    );
    if (restore) {
      const target = state.repositories.find(r => r.id === restore[1]);
      if (!target) return { status: 404, body: { error: "not_found" } };
      target.connection_status = "connected";
      target.verification_status = "pending";
      target.updated_at = new Date().toISOString();
      return { status: 200, body: target };
    }

    // Provider 授权
    const authStart = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/git-providers$/
    );
    if (authStart) {
      return {
        status: 200,
        body: {
          authorization_url: "http://127.0.0.1:8848/mock-provider-authorize"
        }
      };
    }

    // S01-04 启动认知 Run
    const startRun = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/cognition\/runs$/
    );
    if (startRun) {
      const run = {
        id: `run-${Date.now()}`,
        project_id: startRun[1],
        status: "analyzing",
        scope: (payload.scope as string[]) ?? [],
        repository_baselines: [],
        inputs: [],
        capability_version: "cognition-1.2",
        item_count: 0,
        confirmed_count: 0,
        error_message: null,
        started_at: new Date().toISOString(),
        completed_at: null,
        created_at: new Date().toISOString()
      };
      state.runs.unshift(run);
      return { status: 201, body: run };
    }

    const cancelRun = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/cognition\/runs\/([^/]+)\/cancel$/
    );
    if (cancelRun) {
      const run = state.runs.find(r => r.id === cancelRun[2]);
      if (!run) return { status: 404, body: { error: "not_found" } };
      run.status = "cancelled";
      return { status: 200, body: run };
    }

    // S01-01 风险裁决
    const decideRisk = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/proposals\/([^/]+)\/risks\/([^/]+)\/decide$/
    );
    if (decideRisk) {
      const proposal = state.proposals.find(p => p.id === decideRisk[2]);
      if (!proposal) return { status: 404, body: { error: "not_found" } };
      const risk = proposal.risks.find(r => r.risk_id === decideRisk[3]);
      if (!risk) return { status: 404, body: { error: "not_found" } };
      risk.human_decision = String(payload.human_decision ?? "accepted");
      risk.decision_note = (payload.note as string) ?? null;
      risk.decided_by = "prc-zhang";
      risk.decided_at = new Date().toISOString();
      return { status: 200, body: risk };
    }

    // S01-01 要求修订
    const revise = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/proposals\/([^/]+)\/revise$/
    );
    if (revise) {
      const proposal = state.proposals.find(p => p.id === revise[2]);
      if (!proposal) return { status: 404, body: { error: "not_found" } };
      proposal.status = "needs_revision";
      proposal.proposal_version += 1;
      proposal.updated_at = new Date().toISOString();
      return { status: 200, body: proposal };
    }

    // S01-01 拒绝
    const reject = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/proposals\/([^/]+)\/reject$/
    );
    if (reject) {
      const proposal = state.proposals.find(p => p.id === reject[2]);
      if (!proposal) return { status: 404, body: { error: "not_found" } };
      proposal.status = "rejected";
      proposal.rejection_reason = String(payload.reason ?? "");
      proposal.updated_at = new Date().toISOString();
      return { status: 200, body: proposal };
    }

    // S01-01 重新生成
    const regenerate = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/proposals\/([^/]+)\/regenerate$/
    );
    if (regenerate) {
      const proposal = state.proposals.find(p => p.id === regenerate[2]);
      if (!proposal) return { status: 404, body: { error: "not_found" } };
      proposal.status = "pending_review";
      proposal.proposal_version += 1;
      proposal.updated_at = new Date().toISOString();
      return { status: 200, body: proposal };
    }

    // S01-01 正式确认
    const confirm = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/baseline\/proposals\/([^/]+)\/confirm$/
    );
    if (confirm) {
      const proposal = state.proposals.find(p => p.id === confirm[2]);
      if (!proposal) return { status: 404, body: { error: "not_found" } };

      // P0 未裁决 → 阻断
      const unresolved = proposal.risks.filter(
        r => r.risk_level === "P0" && !r.human_decision
      );
      if (unresolved.length > 0) {
        return {
          status: 409,
          body: {
            error: "unresolved_p0_risk",
            risks: unresolved.map(r => r.risk_id)
          }
        };
      }

      // 并发令牌校验
      const current = state.baselines.find(b => b.superseded_at === null);
      const expected = payload.expected_current_baseline_id as string | null;
      if ((current?.id ?? null) !== expected) {
        return { status: 409, body: { error: "baseline_conflict" } };
      }
      if (payload.expected_proposal_version !== proposal.proposal_version) {
        return { status: 409, body: { error: "proposal_version_conflict" } };
      }

      // 同事务：新版本生效，旧版本转历史
      if (current) current.superseded_at = new Date().toISOString();
      const baseline = {
        id: `bl-${Date.now()}`,
        project_id: confirm[1],
        version: (current?.version ?? 0) + 1,
        content: proposal.content,
        repositories: proposal.repository_baselines,
        proposal_id: proposal.id,
        proposal_version: proposal.proposal_version,
        confirmed_by: "prc-zhang",
        confirmed_by_name: "张伟",
        confirmed_at: new Date().toISOString(),
        superseded_at: null,
        created_at: new Date().toISOString()
      };
      state.baselines.push(baseline);
      proposal.status = "confirmed";
      proposal.confirmed_at = baseline.confirmed_at;

      return {
        status: 201,
        body: {
          baseline_id: baseline.id,
          baseline_version: baseline.version,
          current_baseline_id: baseline.id,
          confirmed_by: baseline.confirmed_by,
          confirmed_at: baseline.confirmed_at,
          proposal_id: proposal.id,
          proposal_version: proposal.proposal_version,
          repository_baselines: baseline.repositories,
          confirmation_note: (payload.confirmation_note as string) ?? null
        }
      };
    }
  }

  /* ---------- PATCH ---------- */
  if (method === "PATCH") {
    // S01-04 认知逐项处理
    const decideItem = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/cognition\/items\/([^/]+)$/
    );
    if (decideItem) {
      const item = state.items.find(i => i.id === decideItem[2]);
      if (!item) return { status: 404, body: { error: "not_found" } };
      const action = String(payload.action ?? "");
      if (action === "confirm" || action === "edit_and_confirm") {
        item.status = "confirmed";
        if (action === "edit_and_confirm" && payload.content) {
          item.content = String(payload.content);
        }
        // 确认即形成正式知识
        state.knowledge.unshift({
          id: `kn-${Date.now()}`,
          project_id: item.run_id,
          category: item.domain,
          content: item.content,
          source_references: item.evidence,
          confirmed_by: "prc-zhang",
          confirmed_by_name: "张伟",
          confirmed_at: new Date().toISOString(),
          version: 1,
          status: "active",
          supersedes: null,
          is_core: false,
          created_at: new Date().toISOString()
        });
      } else if (action === "reject") {
        item.status = "rejected";
      } else if (action === "defer") {
        item.status = "deferred";
      } else if (action === "request_investigation") {
        item.status = "investigation_required";
      }
      item.decided_by = "prc-zhang";
      item.decided_at = new Date().toISOString();
      item.decision_note = (payload.note as string) ?? null;
      return { status: 200, body: item };
    }

    // 设/取消核心知识
    const core = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/knowledge\/([^/]+)\/core$/
    );
    if (core) {
      const knowledge = state.knowledge.find(k => k.id === core[2]);
      if (!knowledge) return { status: 404, body: { error: "not_found" } };
      knowledge.is_core = Boolean(payload.is_core);
      return { status: 200, body: knowledge };
    }

    // 项目背景信息更新
    const profileUpdate = pathname.match(
      /^\/v1\/pm\/projects\/([^/]+)\/profile$/
    );
    if (profileUpdate) {
      return {
        status: 200,
        body: {
          project_id: profileUpdate[1],
          project_type: (payload.project_type as string) ?? null,
          business_domain: (payload.business_domain as string) ?? null,
          expected_outcome: (payload.expected_outcome as string) ?? null,
          initial_context: (payload.initial_context as string) ?? null,
          updated_at: new Date().toISOString()
        }
      };
    }
  }

  return undefined;
}
