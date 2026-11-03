p = 'docs/s01-interface-matrix-v1.md'
s = open(p, encoding='utf-8').read()

old = '''| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/cancel` | `cancel_handler` | pm_imports.rs:165 |'''
new = '''| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/cancel` | `cancel_handler` | pm_imports.rs | 支持 `proposed` 态；body 可带 `reason`（写入审计） |
| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/terminate` | `terminate_handler` | pm_imports.rs | **新增（I6-B-05）**：`proposed` 态显式终止，释放 active 名额 |'''
assert old in s, "imports rows"
s = s.replace(old, new, 1)

old = '''| POST | `/v1/pm/projects/{project_id}/imports` | `start_handler` | pm_imports.rs:136 | repo_import |'''
new = '''| POST | `/v1/pm/projects/{project_id}/imports` | `start_handler` | pm_imports.rs | repo_import | **请求体必填 `repository_ids`（N-3）** + 可选 `idempotency_key`（N-4） |'''
assert old in s, "start row"
s = s.replace(old, new, 1)

old = '''| ⑩ | **Discovery Session 与 Task 两套状态机并行** | `discovery/session.rs:161-176` vs Task 13 态，无交叉校验 | I6-B-05 |'''
new = '''| ⑩ | **Discovery Session 与 Task 两套状态机并行** | `discovery/session.rs:161-176` vs Task 13 态，无交叉校验 | I6-B-05 |
| ⑪ | **MCP 权限码只覆盖 5 个写工具**（D-11） | lmms 侧只有 `PM_MCP_READ` / `PM_MCP_DOCUMENT_WRITE` / `PM_MCP_TASK_UPDATE` 三个码，无发现族专用码；单仓收紧会打断现网执行链路 | **跨仓协同**（lmms 发码 → ai-pm 收紧 → 回归），非单仓可决 |'''
assert old in s, "gap 10"
s = s.replace(old, new, 1)

open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = 'docs/s01-acceptance-matrix-v1.md'
s = open(p, encoding='utf-8').read()
old = '''| I1-D（归档） | S01-A061~A065 | 归档后：入口关闭、并发安全、凭据撤销、进程可控、消息安全、审计完整、故障可恢复（**七项须分别证明**，I1 §17.3） |'''
new = '''| I1-D（归档） | S01-A061~A065 | 归档后：入口关闭、并发安全、凭据撤销、进程可控、消息安全、审计完整、故障可恢复（**七项须分别证明**，I1 §17.3） |
| I6-B-05/06（导入契约） | S01-A066~A075 | 导入契约与审计：`repository_ids` 必填（缺省/空 → 422）、跨项目仓库拒绝（422）、导入集合在创建时固化（Source + 首轮 Attempt）、初始化操作被占有、同幂等键重放同一导入（`replayed=true` 且不新建）、`proposed` 态 terminate 后释放名额、`cancel` 带 reason 写入审计、导入启动/初始化占有/终止三类审计行可追溯。**2026-11-03 落地 10 例全绿**（`apps/pm-api/tests/import_contract.rs` 7 例 + 导入既有链路回归） |'''
assert old in s, "acceptance rows"
s = s.replace(old, new, 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

print("registries updated")
