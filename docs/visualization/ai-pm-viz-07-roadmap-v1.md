# 实施路线图及任务拆解 V1.0

> 状态：草案 · 日期：2026-11-12 · 原则：先 Foundation 后场景；每期结束都有可验收增量；不进入开发直到产品评审通过本套文档。

## P0 Visualization Foundation（MVP，建议 3 个并行轨）

| 任务 | 内容 | 依赖 | 验收锚点 |
|---|---|---|---|
| V0-1 Spec 与校验器 | 定稿词表（§评审后）+ JSON Schema + Rust 校验器（E/W/I）+ source_hash 算法；契约测试（双 Adapter + 降级三路） | 评审通过 | D8 AC-1/2 |
| V0-2 存储与 API | 迁移 0020（viz.artifact/approval/render_output/slot_config）+ CRUD/审批/预览端点 + 权限门 + stale 判定 | V0-1 | D8 AC-3/4 |
| V0-3 AI 生成链路 | envelope 新增 `viz_artifact` 类型 + 起草 prompt 规范（含"何时不画图"守则）+ 修补循环 ≤3 轮 + 对话卡 Accept/Reject | V0-1 | D8 AC-5 |
| V0-4 SeeCode Adapter | vendor 锁版本（tools/seecode/，MIT 声明保留）+ 编译器（Spec→SeeCode spec）+ worker 渲染 job（HTML/SVG，零外网配置）+ Node 镜像层 | V0-1, V0-2 | D8 AC-6 |
| V0-5 Viewer 与安全 | ArtifactFrame（sandbox/CSP）+ 对话卡 UI + StaleBar + 降级表格 + PRISM 令牌扩展 + i18n | V0-2 | D8 AC-7/8 |
| V0-6 中文与性能验证 | 中文 spec 用例集（换行/图例/导出字体）+ 规模分档压测（30/150/500）+ 转义注入用例 | V0-4, V0-5 | D8 AC-9/10 |
| V0-7 追踪图 Live MVP | viz_query 聚合层（追踪图一类）+ `/v1/viz/live/trace-graph` + 领域页嵌块 + 降级 | V0-1, V0-2 | D8 AC-11 |
| V0-8 Task DAG Live MVP | dag kind + task_dependency 查询 + 大图分档渲染 | V0-7 | D8 AC-11 |

并行轨划分：轨A=V0-1→V0-3；轨B=V0-4（可并行，只需 V0-1 的 schema 冻结）；轨C=V0-5→V0-6；V0-7/8 在 V0-2 后启动。

## P1 Cognition Visualization

| 任务 | 内容 | 依赖 |
|---|---|---|
| V1-1 Adapter A 选型定稿 | G6 5 引入（验证 CN 文档/包体/lazy chunk）；Vue Flow 作 fallback 决策 | P0 |
| V1-2 能力地图 | capability-map kind + pc.relationship 聚合 + lens | V1-1 |
| V1-3 技术架构图 | architecture-graph + cognition_item/baseline 聚合 | V1-1 |
| V1-4 覆盖关系/影响分析 | impact-tree（反向展开）+ 覆盖缺口标注 | V0-7 |
| V1-5 Artifact ↔ Live 打通 | AI 生成图发布为 slot 后可在 Live 视图中对齐（同 slot 双视图） | V0-3 |

## P2 AI Collaboration Visualization

| 任务 | 内容 | 依赖 |
|---|---|---|
| V2-1 执行时序图 | task_run_event 回放 → sequence kind（幂等事件表已备） | P1 |
| V2-2 Multi-Agent 协作图 | conversation + run_event 聚合 | V2-1 |
| V2-3 图上反哺上下文 | 图选中节点 → 注入 conversation_context_item | P0 |

## P3 Project Intelligence Visualization

| 任务 | 内容 | 依赖 |
|---|---|---|
| V3-1 关键路径 | DAG 上 CPM 计算 + 路径高亮 | P1 |
| V3-2 风险传播 | blocked/awaiting_decision 传播展开（风险定义先过领域评审） | V3-1 |
| V3-3 Commit 影响分析 | commit→task→requirement 链路核验后落 impact-tree | G-4 缺口解除 |
| V3-4 多 Repo 关系 | repository cognition 聚合图 | P1 |

## 明确不做（本期，防蔓延）

- 图形编辑画布 / 实时协同编辑 / 独立渲染微服务 / BI 报表 / 移动端专门布局 / 图形 Git 化（D3 §9.3）。

## 里程碑门槛

| 门槛 | 内容 |
|---|---|
| M1（P0 完） | 三个 MVP 场景上线灰度；全量既有测试绿 + viz 可拔测试绿；安全用例全过 |
| M2（P1 完） | 三张认知图可用；G6 性能达标（150 节点 60fps 交互） |
| M3（P2 完） | 时序回放图可用；上下文反哺打通 |
| M4（P3 完） | 智能分析类图按数据成熟度逐个放行 |

## 评审前置条件（进入开发前必须完成）

1. 本套 9 份文档产品评审通过（尤其 D3 §9 三个开放问题、D5 缺口 G-3 处置）。
2. `kind/rel` 词表各领域 owner 会签。
3. 安全评审确认 sandbox/CSP 方案（D4 §5）与内网导出策略。
4. 迁移号确认（0020 起）与台账登记责任人指定。
