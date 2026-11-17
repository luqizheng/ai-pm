import type { Component } from "vue";
import {
  Activity,
  BookOpen,
  Brain,
  Folders,
  Home,
  Users
} from "lucide-vue-next";

/**
 * Main Navigation V2.0 —— 三级导航信息架构（设计原型数据，非生产单源）。
 *
 * L1 业务域（Domain）→ L2 功能模块（Module）→ L3 功能页（Page）。
 * 本文件只服务 /design/navigation/v2 目录板；业务落地仍以
 * src/router/navigation.ts + locales 为单源（设计参考文本，中文标注）。
 */

/** 原型角色：member = 普通成员，admin = 团队管理员 */
export type NavV2Role = "member" | "admin";

/** L3 功能页：真实目的地 */
export interface NavV2Page {
  /** 稳定标识（原型内唯一，形如路径） */
  id: string;
  label: string;
  /** 徽标数字 */
  badge?: number;
  /** 需要授权的角色；缺省全角色可用。命中失败渲染受限态（锁 + 置灰） */
  roles?: NavV2Role[];
  /** 规划占位页（渲染 PLANNED 标识，不伪装已上线） */
  planned?: boolean;
}

/** L2 功能模块：领域内的功能分组，可折叠 */
export interface NavV2Module {
  id: string;
  /** 空标签 = 领域直属页组（无模块标题行，不渲染重复组头） */
  label: string;
  pages: NavV2Page[];
}

/** L1 业务域：一级导航，常驻可见 */
export interface NavV2Domain {
  id: string;
  label: string;
  icon: Component;
  /** 可见角色；缺省 = 全角色可见（角色外整体隐藏） */
  roles?: NavV2Role[];
  modules: NavV2Module[];
}

/** A 个人工作台 —— 领域直属页（无中间模块层） */
const personalDomain: NavV2Domain = {
  id: "personal",
  label: "个人工作台",
  icon: Home,
  modules: [
    {
      id: "personal-root",
      label: "",
      pages: [
        { id: "/v2/personal/overview", label: "个人概览" },
        { id: "/v2/personal/tasks", label: "我的任务", badge: 5 },
        { id: "/v2/personal/approvals", label: "我的审批", badge: 2 },
        { id: "/v2/personal/activity", label: "我的动态" }
      ]
    }
  ]
};

/** B 项目管理 —— 三级最深的领域（4 模块 × 3~4 页） */
const projectDomain: NavV2Domain = {
  id: "project",
  label: "项目管理",
  icon: Folders,
  modules: [
    {
      id: "project-list",
      label: "项目列表",
      pages: [
        { id: "/v2/project-list/all", label: "全部项目" },
        { id: "/v2/project-list/recent", label: "最近访问" },
        { id: "/v2/project-list/archived", label: "已归档项目" }
      ]
    },
    {
      id: "project-execution",
      label: "项目执行",
      pages: [
        { id: "/v2/execution/command-center", label: "项目总控台" },
        { id: "/v2/execution/tasks", label: "任务管理" },
        { id: "/v2/execution/risks", label: "项目风险" },
        { id: "/v2/execution/activity", label: "项目动态" }
      ]
    },
    {
      id: "project-knowledge",
      label: "项目知识",
      pages: [
        { id: "/v2/project-knowledge/cognition", label: "项目认知" },
        { id: "/v2/project-knowledge/documents", label: "项目文档" },
        { id: "/v2/project-knowledge/decisions", label: "项目决策" }
      ]
    },
    {
      id: "project-config",
      label: "项目配置",
      pages: [
        { id: "/v2/project-config/repositories", label: "仓库管理" },
        { id: "/v2/project-config/devenv", label: "开发环境" },
        { id: "/v2/project-config/settings", label: "项目设置" }
      ]
    }
  ]
};

/** C 团队知识 —— 含空模块占位（知识组织待建） */
const teamKnowledgeDomain: NavV2Domain = {
  id: "team-knowledge",
  label: "团队知识",
  icon: BookOpen,
  modules: [
    {
      id: "knowledge-base",
      label: "知识库",
      pages: [
        { id: "/v2/kb/overview", label: "知识总览" },
        { id: "/v2/kb/documents", label: "知识文档" },
        { id: "/v2/kb/organization", label: "知识组织", planned: true },
        { id: "/v2/kb/search", label: "知识检索" }
      ]
    }
  ]
};

/** D 产品认知 —— 混合层级：直属页 + 3 个模块 */
const productCognitionDomain: NavV2Domain = {
  id: "product-cognition",
  label: "产品认知",
  icon: Brain,
  modules: [
    {
      id: "product-root",
      label: "",
      pages: [{ id: "/v2/product/overview", label: "产品总览" }]
    },
    {
      id: "product-structure",
      label: "产品结构",
      pages: [
        { id: "/v2/product/domains", label: "业务域" },
        { id: "/v2/product/modules", label: "产品模块" },
        { id: "/v2/product/capabilities", label: "能力" }
      ]
    },
    {
      id: "product-requirements",
      label: "产品需求",
      pages: [
        { id: "/v2/product/requirements", label: "需求清单" },
        { id: "/v2/product/rules", label: "业务规则" },
        { id: "/v2/product/scenarios", label: "用户场景" }
      ]
    },
    {
      id: "product-management",
      label: "认知管理",
      pages: [
        { id: "/v2/product/pending", label: "待确认", badge: 7 },
        { id: "/v2/product/issues", label: "认知问题" },
        { id: "/v2/product/changes", label: "变更历史" }
      ]
    }
  ]
};

/** E 分析中心 —— 领域直属页 + 受限页示例（分析历史需管理员） */
const analysisDomain: NavV2Domain = {
  id: "analysis",
  label: "分析中心",
  icon: Activity,
  modules: [
    {
      id: "analysis-root",
      label: "",
      pages: [
        { id: "/v2/analysis/overview", label: "分析总览" },
        { id: "/v2/analysis/jobs", label: "分析任务" },
        { id: "/v2/analysis/results", label: "分析结果" },
        { id: "/v2/analysis/history", label: "分析历史", roles: ["admin"] }
      ]
    }
  ]
};

/** F 团队管理 —— 仅管理员可见的领域 */
const teamAdminDomain: NavV2Domain = {
  id: "team-admin",
  label: "团队管理",
  icon: Users,
  roles: ["admin"],
  modules: [
    {
      id: "team-admin-root",
      label: "",
      pages: [
        { id: "/v2/admin/overview", label: "团队概览" },
        { id: "/v2/admin/members", label: "成员与邀请" },
        { id: "/v2/admin/roles", label: "角色与权限" },
        { id: "/v2/admin/settings", label: "团队设置" },
        { id: "/v2/admin/ai", label: "AI 配置" }
      ]
    }
  ]
};

/** 全量 IA（V2.0 六域）；角色过滤由渲染层按 role 处理 */
export const navV2Domains: NavV2Domain[] = [
  personalDomain,
  projectDomain,
  teamKnowledgeDomain,
  productCognitionDomain,
  analysisDomain,
  teamAdminDomain
];

/** 项目上下文导航（§5：进入项目后的独立二级导航，不含团队知识/产品认知） */
export const navV2ProjectDomains: NavV2Domain[] = [
  {
    id: "ctx-project",
    label: "项目导航",
    icon: Folders,
    modules: [
      {
        id: "ctx-root",
        label: "",
        pages: [{ id: "/v2/p/command-center", label: "项目总控台" }]
      },
      {
        id: "ctx-execution",
        label: "项目执行",
        pages: [
          { id: "/v2/p/tasks", label: "任务管理" },
          { id: "/v2/p/risks", label: "项目风险" }
        ]
      },
      {
        id: "ctx-knowledge",
        label: "项目知识",
        pages: [
          { id: "/v2/p/cognition", label: "项目认知" },
          { id: "/v2/p/documents", label: "项目文档" },
          { id: "/v2/p/decisions", label: "项目决策" }
        ]
      },
      {
        id: "ctx-config",
        label: "项目配置",
        pages: [
          { id: "/v2/p/repositories", label: "仓库管理" },
          { id: "/v2/p/devenv", label: "开发环境" },
          { id: "/v2/p/settings", label: "项目设置" }
        ]
      }
    ]
  }
];

/** 按 id 深搜页 → 返回「域 / 模块 / 页」轨迹（面包屑用）；找不到返回 null */
export function findNavV2Trail(
  domains: NavV2Domain[],
  pageId: string
): { domain: NavV2Domain; module: NavV2Module; page: NavV2Page } | null {
  for (const domain of domains) {
    for (const module of domain.modules) {
      const page = module.pages.find(item => item.id === pageId);
      if (page) return { domain, module, page };
    }
  }
  return null;
}

/** 角色可见性判定：域级与页级统一入口 */
export function isNavV2Allowed(
  item: { roles?: NavV2Role[] },
  role: NavV2Role
): boolean {
  return !item.roles || item.roles.includes(role);
}
