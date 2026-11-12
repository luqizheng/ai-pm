# 技术风险与验收标准 V1.0

> 状态：草案 · 日期：2026-11-12 · 分级：S0=阻断发布 / S1=发布前必修 / S2=限期修复 / S3=挂账。

## 1. 风险登记表

| # | 风险 | 级别 | 场景 | 缓解 | 残余风险处置 |
|---|---|---|---|---|---|
| R-1 | 模型编造业务关系（幻觉节点/边） | S0 | AI 生成链路 | 生成侧对象 ID 白名单（仅限 Context Item/工具查询结果）；校验器 E 级拒绝未知引用；溯源强制 | 溯源覆盖率指标监控（D1 §8），<90% 触发生成 prompt 回顾 |
| R-2 | 图被当作知识权威（审批旁路） | S0 | 全部 | 图形零业务写路径（引擎无任何领域写端点）；UI 恒显 "未生效" 语义；关系修正跳转原领域 | 代码评审 checklist 固定项：viz 模块禁止 import 领域写 service |
| R-3 | Artifact HTML XSS / 沙箱逃逸 | S0 | Artifact 通道 | sandbox iframe（无 same-origin）+ 独立 CSP + spec 白名单 + 注入用例集（AC-9）；SeeCode 转义行为未承诺 → 以测试事实为准，不过则升级为 S0 阻断 | 每次升级 vendor 版本重跑注入用例 |
| R-4 | SeeCode 弃维/破坏性变更 | S1 | Adapter B | vendor 锁版本 + 编译器单点隔离 + spec 双 Adapter 契约测试 | 弃维预案：产物通道降级为表格/改 Mermaid，2 周内可切换 |
| R-5 | 私有知识外泄（外网请求） | S0 | Artifact 通道 | `fonts:system` 零外网；服务端不装浏览器；内网禁页面内位图导出；出域请求断言测试（AC-8） | health 端点记录 vendor 版本与外网开关状态 |
| R-6 | 权限绕过（图泄露无权对象） | S0 | Live+Artifact | 服务端裁剪（非前端隐藏）；404 同形；api_key 403；越权引用 E 级 | 渗透用例进入 E2E（无权项目成员请求图 → 404 同形断言） |
| R-7 | 大图性能塌方 | S1 | Live | 规模分档 S/M/L + >500 拒绝；视口裁剪/LOD/聚合；viz_query 分页探索 | 压测入 CI 基线（150 节点交互帧率） |
| R-8 | 中文渲染缺陷（换行/字体/导出乱码） | S1 | 双通道 | 中文用例集进验收（AC-10）；SVG 内嵌字体；CJK 换行实测 | 缺陷进 D5 场景回归 |
| R-9 | Stale 误判/漏判 | S1 | Artifact | source_hash 覆盖 revision_id+content_hash；单测指纹变更矩阵 | 抽样审计 stale 标记 |
| R-10 | 引擎故障拖垮业务 | S0 | 全部 | lazy chunk + feature flag + "viz 可拔"回归测试（关分区全量绿） | 灰度期监控降级命中率 |
| R-11 | G6 包体拖慢首屏 | S2 | P1 | 独立 lazy chunk + 按需注册（tree-shaking）；bundle 门禁阈值 | 评审时定 chunk 体积上限 |
| R-12 | 词表失控（rel 泛滥成 free-text） | S2 | Spec | 词表登记制 + lint 白名单校验 + owner 会签 | 季度词表清理 |

## 2. 验收标准（P0 门槛，全部可测）

### 功能
- **AC-1**：固定 spec 样例集通过校验器（合法样例全过；注入非法样例按 E/W/I 分级返回预期 `{code,at,fix}`）。
- **AC-2**：每个 view.kind 至少 2 个渲染目标出图成功 + 降级表格可渲染（契约测试自动化）。
- **AC-3**：artifact 版本机全路径（draft→published→stale；draft→rejected）状态与权限正确；pending 审批唯一约束触发 409。
- **AC-4**：spec 引用的源对象 revision 变更后，artifact 在下一次判定中置 stale（指纹变更矩阵单测）。
- **AC-5**：对话内生成→修补→Accept→发布 全链路 E2E；Reject 必带 reason；Reject 后图不可见。

### 安全
- **AC-6**：渲染产物零外网请求（网络层断言：无 Google Fonts/jsDelivr 连接）。
- **AC-7**：sandbox iframe 不携带 allow-same-origin；产物 JS 访问 parent/localStorage/cookie 全部失败（自动化断言）。
- **AC-8**：权限矩阵用例：跨项目/越权对象/api_key 三类请求均得到预期 404 同形或 403。
- **AC-9**：注入用例集：label 含 `<script>`、`onerror=`、`javascript:`、`{{` 模板注入 → 渲染安全或 E 级拒绝。

### 质量与性能
- **AC-10**：中文用例集：长中文标签换行、图例中文、SVG 导出内嵌字体、深色主题一致。
- **AC-11**：规模分档：30/150/500 节点档位行为符合 D1 §6；>500 拒绝并提示；150 节点交互帧率达标（P0 定基线）。
- **AC-12**：viz 可拔：feature flag 关闭 viz 分区，全量既有测试套件绿（进 CI 常态）。
- **AC-13**：零 mock 合规：发布构建无 fixtures/demo/mock 模块引用（release-guard 既有门禁通过）。
- **AC-14**：i18n：`viz.*` 命名空间完整，`check:i18n` 双脚本通过。

### 追溯与产品原则
- **AC-15**：任意节点点击可见 source_ref 且可跳转原文档；无溯源对象必有 `unlinked` 灰标（UI 断言）。
- **AC-16**：接受图形后，数据库中无任何业务对象被修改（审计断言）——P1/P3 原则的最终测试。

## 3. 验收责任与回归节奏

| 项 | 安排 |
|---|---|
| AC-1~5/12/16 | Rust 集成测试 + vitest（每 PR） |
| AC-6~9（安全类） | 专项 E2E（playwright，独立 config），vendor 升级与发版必跑 |
| AC-10/11 | 里程碑门槛跑（M1 起每期） |
| 指标类（D1 §8） | 上线灰度期观察，M2 评审复盘 |

## 4. 决策记录钩子

- D32/D38 等既有编号沿用语义：本套文档裁决编号建议启用 **DV-**（Design Visualization）前缀，首个为 DV-1（引擎定位=基础设施非独立域）与 DV-2（双 Adapter 架构），登记于 00 索引。
