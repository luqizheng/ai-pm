# smark-pm 组织关系与权限模型资料包

**资料包：** `/smark-pm/architecture/organization-access-v1/`  
**更新：** 2026-11-21  
**状态：** 决策对齐资料已整理；代码级验收和最终冻结尚未完成。

## 阅读顺序

1. `P4-ORG-PA-决策效力与修订关系总表-v1.0.md`：先理解 DR-ORG → DR-PA 的正式效力关系、OPEN 与历史材料边界。
2. `P4-ORG-PA-组织关系与权限模型正式基线-v1.0.md`：理解目标领域模型；这是重建稿，需结合决策总表阅读。
3. `P4-ORG-PA-实施契约与验收基线-v0.2.md`：工程核验和验收要求。
4. `P4-ORG-PA-冲突与OPEN核验台账-v0.2.md`：剩余 OPEN、P4 规划修订和代码核验任务。
5. 原始签发文件：`P4-ORG-修订决策记录-v1.0.md`、`P4-PA-修订决策记录-v1.0.md`、`smark-pm-pa-product-access-decisions-2026-11-19.md`、`P4-ORG-改造计划-v0.3.md`。

## 权威纪律

- 原始 DR 是正式决策来源；总表只做索引，不取代 DR。
- P4-PA 明确修订 DR-ORG-03/04/05/06/07 的相关部分；DR-ORG-01/02/08/09 继续有效。
- PA §8 七项 OPEN 未经正式裁决不得实现。
- P4-09 敏感区历史草案中的 PROPOSED 不自动升级为 ACCEPTED/FROZEN。
- DR-ORG-09 禁止本轮迁移、双写、兼容分支和过渡层。
- Team 不可解散；“dissolve 全链删除”是删除解散功能链，不是解散 Team。
- 实施计划声称完成不等于代码级独立审计通过。
