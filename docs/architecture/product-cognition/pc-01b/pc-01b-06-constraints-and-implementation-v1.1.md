# PC-01B — 约束与实施（V1.2 拆分件 06，原文档 §14~§22）

> **拆分声明**：本文件为《PC-01B 数据模型与数据库设计》拆分件之一（内容基线 V1.1 拆分，**V1.2 差异复核落版 2026-11-07**：§14.2 矩阵 17 表、§15.2 表清单、§17.3 锁序尾项 outbox、§18.1 ⑫b 事件回写、§20 T-01 增强 + T-30/31、§22.2 建表清单与顺序），内容为原文档 §14~§22；章节编号保持原文档编号。总纲/裁决/验收（§23/§24/附录 A）见拆分件 00。

---

# 14. 租户隔离与完整性约束

## 14.1 隔离模型

〔事实〕0019 统一模式：`UNIQUE (tenant_id, id)`（租户级身份键）+ `(tenant_id, project_id) → core.project (tenant_id, id)` 组合 FK（0019:149-168, 235-258）。PC 在此之上增加**项目级身份键** `UNIQUE (tenant_id, project_id, id)`，使全部 PC 内部复合 FK（成员/端点/引用）都能把 project 校验下沉到库层——修复 M-8 型「单列 FK 不强制同项目」缺陷。

## 14.2 全表隔离约束矩阵（17 张新表逐一声明；V1.2/C-1 增文档承载两表）

| 表 | tenant_id | project 组合 FK | UNIQUE(tenant_id,id) | UNIQUE(tenant_id,project_id,id) | 内部隔离复合 FK |
|---|---|---|---|---|---|
| pc.object_type / pc.relationship_type | —（平台级字典） | — | — | — | — |
| pc.document（V1.2/C-1 正文承载） | ✓ | ✓ | ✓ | ✓ | —（被 cognition_revision 组合 FK 引用） |
| pc.document_revision（V1.2/C-1） | ✓ | ✓ | ✓ | ✓ | document（项目级组合 FK，含组合一致性 revision ∈ document） |
| pc.cognition_object | ✓ | ✓ | ✓ | ✓ | current/superseded 自引用 |
| pc.cognition_revision | ✓ | ✓ | ✓ | ✓ | object；**pc.document\*（项目级组合 FK，C-2）** |
| pc.relationship | ✓ | ✓ | ✓ | ✓ | 端点 object ×2；superseded 自引用 |
| pc.relationship_revision | ✓ | ✓ | ✓ | ✓ | relationship；端点 object+revision ×2（§6.4 四复合）；可选关系正文→pc.document\*（同 C-1） |
| pc.cognition_version | ✓ | ✓ | ✓ | ✓ | proposal；predecessor 自引用 |
| pc.version_member | ✓ | ✓ | ✓（tenant+id） | —（成员唯一性由两把部分唯一索引承担，§7.3） | version；object+revision；relationship+revision（四复合） |
| pc.proposal | ✓ | ✓ | ✓ | ✓ | source（可空） |
| pc.proposal_member | ✓ | ✓ | ✓（tenant+id） | — | proposal；object/relationship 四复合 |
| pc.approval | ✓ | ✓ | ✓ | ✓ | proposal |
| pc.conflict | ✓ | ✓ | ✓ | ✓ | — |
| pc.conflict_object | ✓ | ✓ | —（id PK + `uq_pc_conflict_object` 表达式唯一索引，§10.3 V1.1 R1） | — | conflict；object/relationship |
| pc.source | ✓ | ✓ | ✓ | ✓ | source；**跨域四元组（knowledge_base 等，服务层+对账，C-4/V1.2）** |
| pc.evidence | ✓ | ✓ | ✓ | ✓ | source；object_revision / relationship_revision；**跨域四元组（C-4/V1.2）** |
| pc.write_operation | ✓ | ✓ | —（UNIQUE(tenant_id,project_id,operation_id) 承载幂等） | — | 产出回填列（可空，经状态机约束） |

结论：**同一正式版本不可能跨项目引用对象**（Member 复合 FK 含 project_id 且父键为项目级身份键）；**Source/Evidence/Proposal/Approval/Conflict 全部经组合 FK 锚定同项目**；**PC 正文引用（cognition_revision → pc.document\*）为项目级组合 FK 且库层保证组合一致性（C-2 约束增强）**；跨域引用（KB/需求等）走四元组+服务层校验（C-4）。隔离不依赖应用层过滤（任务书 §13）。

## 14.3 跨租户/跨项目攻击面与拦截点

| 攻击面 | 拦截 |
|---|---|
| Member 引用其他项目的 Revision | 复合 FK（tenant+project+object+revision 四列）→ FK violation |
| Relationship Revision 端点跨项目 | 端点四复合 FK |
| Evidence 挂到其他项目的 Revision | 复合 FK |
| Conflict 圈定其他项目对象 | conflict_object 复合 FK |
| Proposal 目标跨项目 | proposal_member 复合 FK |
| PC 引用外域对象跨项目（requirement/project_goal/knowledge_base 等） | 服务层组合键校验 + 对账任务（§13.3/C-4；跨服务表间禁止 FK）；**PC 正文跨项目已由 pc.document\* 项目级 FK 库层拦截（C-2 约束增强，原「PC 引用文档跨项目」攻击面消除）** |

---

# 15. DDL 级设计汇总

## 15.1 Schema 与命名规范

| 项 | 规范 | 依据 |
|---|---|---|
| Schema | `pc`（独立逻辑命名空间；与 ai/governance/delivery 平级） | P7 逻辑前缀惯例；避免 ai.* 旧域语义混淆 |
| 表名 | 单数、snake_case、无前缀（schema 即前缀）：pc.cognition_object | P7 |
| 主键 | `id UUID PRIMARY KEY DEFAULT gen_random_uuid()` | 0001+ 全仓惯例 |
| 时间 | `TIMESTAMPTZ NOT NULL DEFAULT now()` | 全仓惯例 |
| 枚举 | TEXT + CHECK（全仓无 ENUM 类型对象，Gate 0 F.1-3），值 UPPER 与否遵循既有小写先例（0036/0037 均 snake_case 小写值） | 既有惯例 |
| 索引命名 | `idx_pc_*` / `uq_pc_*` / `trg_pc_*` / `fk_pc_*` | P7 idx_/uq_ 前缀 |
| 约束命名 | `chk_pc_*`（CHECK 具名） | 0039 chk_requirement_* 先例 |
| UUID 主键策略 | 库端 gen_random_uuid()（应用层不需要预生成；幂等语义由 operation_id 承担） | A8 模式 |
| JSONB 边界 | 仅 pc.relationship_revision.properties 一处；身份/端点/成员/审批事实/证据定位一律结构化列 | 任务书 §15 |
| Migration | 0045 起号（§21.1 源码级验证支持）；DDL + 14+N 行类型种子 INSERT；单迁移或按「注册表/六核心表/治理表/追溯表」四段拆分由 01C 定（01A §7.3 风险项） | §21.1 |

## 15.2 表清单（17 张；V1.2/C-1 +2 正文承载表）

```text
类型注册：pc.object_type / pc.relationship_type
正文承载：pc.document / pc.document_revision（V1.2/C-1 新增；PCGIT 独立根）
核心六表：pc.cognition_object / pc.cognition_revision / pc.relationship /
          pc.relationship_revision / pc.cognition_version / pc.version_member
治理：    pc.proposal / pc.proposal_member / pc.approval / pc.conflict / pc.conflict_object
追溯：    pc.source / pc.evidence
账本：    pc.write_operation
```

## 15.3 公共触发器函数与视图

```sql
-- append-only 行禁止改删（0026:257-262 同构）
CREATE FUNCTION pc.forbid_row_update() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'row % in %.% is immutable', OLD.id, TG_TABLE_SCHEMA, TG_TABLE_NAME;
END $$;

-- 版本头锚定保护（0026:273-296 同构）
CREATE FUNCTION pc.forbid_version_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (OLD.version_no IS DISTINCT FROM NEW.version_no)
       OR (OLD.confirmed_by IS DISTINCT FROM NEW.confirmed_by)
       OR (OLD.confirmed_at IS DISTINCT FROM NEW.confirmed_at)
       OR (OLD.source_proposal_id IS DISTINCT FROM NEW.source_proposal_id)
       OR (OLD.snapshot_hash IS DISTINCT FROM NEW.snapshot_hash)
       OR (OLD.predecessor_version_id IS DISTINCT FROM NEW.predecessor_version_id)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'pc.cognition_version % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'published' AND NEW.status NOT IN ('published','archived') THEN
        RAISE EXCEPTION 'pc.cognition_version % status may only move published -> archived', OLD.id;
    END IF;
    RETURN NEW;
END $$;

-- Revision 版本化状态派生视图（§5.3 注记 1：不冗余 revision_status）
CREATE VIEW pc.v_revision_versioned AS
SELECT r.tenant_id, r.project_id, r.object_id, r.id AS object_revision_id,
       (vm.object_revision_id IS NOT NULL) AS in_published_version
FROM pc.cognition_revision r
LEFT JOIN LATERAL (
    SELECT 1 FROM pc.version_member vm
    WHERE vm.tenant_id = r.tenant_id AND vm.project_id = r.project_id
      AND vm.object_revision_id = r.id LIMIT 1
) vm ON true;

-- 〔V1.1（R10/F-10）〕身份行 DELETE 防护：object/relationship 仅允许删除
-- 「identity 态且尚无任何 Revision」的初始化残留行；version 全禁（双保险，
-- 主防护仍为 member FK）。
CREATE FUNCTION pc.forbid_identity_delete() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.lifecycle_status <> 'identity' THEN
        RAISE EXCEPTION 'pc.% % is not identity; deletion is forbidden', TG_TABLE_NAME, OLD.id;
    END IF;
    RETURN OLD;
END $$;
CREATE TRIGGER trg_pc_object_delete BEFORE DELETE ON pc.cognition_object
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_identity_delete();
CREATE TRIGGER trg_pc_relationship_delete BEFORE DELETE ON pc.relationship
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_identity_delete();
CREATE TRIGGER trg_pc_version_delete BEFORE DELETE ON pc.cognition_version
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();
-- 认知服务创建 Revision 后 identity 白名单自然失效（有 Revision 的行由子表 FK 先行拒绝）。
```

## 15.4 IMMUTABLE 函数注记

`pc.object_type_singleton()`（§5.2 部分唯一索引谓词）为 IMMUTABLE 函数——部分索引谓词不可查表。〔V1.1（R12/F-12）定稿〕函数体以硬编码单例键集合实现（`type_key IN ('product_profile','product_vision')`，DDL 见 §5.2），规避跨表谓词限制；字典 singleton 列与函数清单的一致性由 §5.1 启动校验承担。类型字典仅在种子迁移中变化（PC-D-04：运行期冻结），字典变更必然伴随该函数与索引的同步迁移。

## 15.5 种子数据

`pc.object_type` 14 行（§3.3）+ `pc.relationship_type` **17 条注册种子**（〔V1.1（R8/F-08）〕按基线 §7.2 矩阵逐条落地，01C 直接执行；矩阵另 2 条 `derived_from`/`supported_by` 的目标为 Source/Evidence——追溯实体而非 Cognition Object，**不注册为关系类型**，由 pc.source/pc.evidence 专用结构承载，避免追溯链复刻为关系）。跨域端点类型以 `domain.type` 形态存于端点类型集合（如 `project_goal.project_goal`、`requirement.requirement_revision`），服务层按 §13.1 通道状态表校验。种子 INSERT 属 DDL 交付；无任何业务数据回填（零迁移边界）。

```sql
-- type_key            | dir       | source_object_types                              | target_object_types                              | gov | self_ref
INSERT INTO pc.relationship_type VALUES
('supports',        'directed', '{product_vision,interaction}',                    '{project_goal.project_goal,scenario}',           'L1', false),
('covers',          'directed', '{product_vision}',                                '{business_domain}',                              'L1', false),
('contains',        'directed', '{business_domain,feature_module,page}',           '{business_capability,feature,interaction}',      'L1', false),
('realized_by',     'directed', '{business_capability,scenario}',                  '{feature_module,business_process}',              'L1', false),
('specified_by',    'directed', '{feature}',                                       '{requirement.requirement_revision}',             'L1', false),
('participates_in', 'directed', '{actor}',                                         '{scenario}',                                     'L1', false),
('uses',            'directed', '{scenario}',                                      '{feature}',                                      'L1', false),
('governed_by',     'directed', '{business_process,domain_object}',                '{business_rule}',                                'L2', false),
('implements',      'directed', '{page}',                                          '{feature}',                                      'L1', false),
('constrains',      'directed', '{business_rule}',                                 '{requirement.requirement_revision}',             'L2', false),
('affects',         'directed', '{product_decision}',                              '{product_profile,product_vision,business_domain,business_capability,feature_module,feature,actor,scenario,business_process,page,interaction,domain_object,business_rule,product_decision}', 'L2', false),
('informs',         'directed', '{product_decision}',                              '{requirement.requirement_revision}',             'L2', false);
```

注记：矩阵 19 行中 `contains`（Domain→Capability / Module→Feature / Page→Interaction）与 `supports`（Vision→Goal / Interaction→Scenario）按端点集合合并为同一 type_key 承载（端点合法性由集合枚举区分）；`derived_from`/`supported_by` 由 §9 追溯结构承载（理由如上）。`allow_duplicate` 全部 false（§6.3 注 4）；`affects` 允许指向全部 14 类对象（决策影响面）。若 01C 实施时需拆分 contains 为三条独立类型（contains_capability/contains_feature/contains_interaction），属等价重构，不改变约束语义。

---

# 16. 索引与查询设计

## 16.1 核心查询 → 索引映射

| # | 查询 | 支撑索引 | 性能注记 |
|---|---|---|---|
| Q1 | 项目全部正式认知对象（最新版本视角） | `idx_pc_vm_version` + `idx_pc_vm_object`；版本解析 `uq_pc_...version_no` 唯一索引 | 成员集合一次取回；对象详情按 id 批量点查 PK |
| Q2 | Object 当前 Revision | `pc.cognition_object` 行内 `current_revision_id` + revision PK 点查 | O(1) |
| Q3 | Object 历史 Revision | `idx_pc_revision_object (tenant_id, project_id, object_id, revision_no DESC)` | 覆盖排序 |
| Q4 | 对象入向/出向 Relationship（活跃） | `idx_pc_relationship_source` / `idx_pc_relationship_target`（含 lifecycle_status） | 跨域端点对象不在索引内（入向跨域查询由四元组反查，低频，§16.2） |
| Q5 | Relationship 历史 Revision | `idx_pc_rel_revision_rel (... relationship_id, revision_no DESC)` | 覆盖排序 |
| Q6 | Version 全部成员 | `idx_pc_vm_version` | 成员集合全量读 |
| Q7 | Version 内全部 Object Revision | `idx_pc_vm_object`（version 维度经 Q6 后点查）或直接 `idx_pc_vm_object` | 双维度各自过滤 |
| Q8 | Version 内全部 Relationship Revision | `idx_pc_vm_rel` | 同上 |
| Q9 | 端点闭合校验 | §8.3 断言：走 `idx_pc_vm_rel` + `idx_pc_vm_version` + Member 部分唯一索引（EXISTS 点查） | 发布事务内一次全量校验，O(关系成员 × 端点 2)；数量级可接受 |
| Q10 | 两 Version 双维度 diff | `(SELECT object_id, object_revision_id FROM vm WHERE version=v1 EXCEPT SELECT ... v2)` 对称差 | 部分唯一索引覆盖列；结果集 = 变更量 |
| Q11 | Proposal 与审批历史 | `idx_pc_proposal_status` + `idx_pc_approval_proposal` | 时间倒序 |
| Q12 | Conflict 与受影响对象 | `idx_pc_conflict_status` + `idx_pc_conflict_object` / `idx_pc_conflict_rel` | 冻结校验为点查集 |
| Q13 | Source/Evidence 完整追溯链（对象→证据→来源→提案） | `idx_pc_evidence_subject`（反向）+ evidence→source 复合 FK 点查 + proposal 经 source_id/成员反查 | 全链 ≤ 4 跳；修复 M-4 反向缺失 |
| Q14 | Git-PG 未完成对账操作 | `idx_pc_write_op_pending`（部分索引，仅扫描未终态行） | 行数小 |

## 16.2 性能风险注记

1. **闭合触发器成本**：版本成员数 N、关系成员占比 k → 校验 O(kN) 次索引点查。10⁴ 成员、k=30% → 3×10³ 点查，单事务毫秒~十毫秒级；若 01C 压测超阈值，降级方案 = 语句级 AFTER STATEMENT 触发器（§8.3 注记 4，语义不变）。
2. **Member 双唯一索引写放大**：每版本批量 INSERT 走两把部分唯一索引（各自维度命中，互不冗余），可接受。
3. **Q4 跨域入向查询**：跨域端点为 TEXT 四元组，无法索引 PC 对象 id（本来也不存在）。PC 内对象被外部域反向引用的检索属外域职责（Plan→PC 由 Plan 域发起，Q-PC-13）。
4. **v_revision_versioned 视图**：LATERAL EXISTS 走部分唯一索引，点查级；仅治理查询使用，不进高频路径。

---

# 17. 并发控制

## 17.1 总策略（A6 CAS 双令牌 + 行锁 + 唯一约束三层）

| 层 | 机制 | 落点 |
|---|---|---|
| 1 行锁 | 关键竞争行 `SELECT ... FOR UPDATE`（Revision 推号、审批裁决、发布入口） | 服务层事务内，锁序固定（§17.3） |
| 2 CAS 令牌 | `expected_version_no`（Proposal/Version）与 `(current_revision_id, updated_at)` 双令牌（Object/Relationship 内容指针） | 服务层 WHERE 条件携带 + 影响行数断言（禁止「先查后改」非原子模式，〔裁决〕基线 §5.6） |
| 3 库层唯一约束 | 版本号、Revision 号、pending 审批、活跃关系身份、幂等键——锁失效时的最后防线 | §15 各表 UNIQUE/部分唯一索引 |

## 17.2 六个竞争场景（任务书 §14.2）

| # | 场景 | 设计行为 |
|---|---|---|
| 1 | 两用户同时修改同一 Object | 事务先 `FOR UPDATE` 对象行 → 推号 `revision_no = max+1` → INSERT Revision → CAS 回填 current（WHERE current_revision_id = 读到的旧值 AND updated_at = 读到的旧值；影响行数=0 → 409 令牌过期）。兜底：`UNIQUE(tenant_id,project_id,object_id,revision_no)` |
| 2 | 两用户同时修改同一 Relationship | 同场景 1（关系行锁 + 关系 Revision 唯一约束）；端点身份被锚定触发器保护，无端点竞争面 |
| 3 | 两个 Proposal 同时基于同一旧 Revision | **允许并行提案**（both expected_version_no = Vₙ）。互斥发生在发布：先发布者占用 Vₙ₊₁；后发布者步骤② CAS 失败 → proposal 转 failed（附失败原因），需基于新版本重新分析后重新提案（〔裁决〕基线 §5.6 冲突失败后的重新分析机制） |
| 4 | 两审批人同时处理同一审批 | `FOR UPDATE` 审批行 + `uq_pc_approval_pending`；先裁决者提交终态，后裁决者读到非 pending → 409（库层双保险：即使应用层漏锁，第二个 UPDATE 被单向触发器拒绝） |
| 5 | 两个版本发布同时发生 | 项目级 advisory lock（`pg_advisory_xact_lock(hashtext('pc_publish:'||project_id))`）串行化发布入口；`UNIQUE(tenant_id,project_id,version_no)` + expected_version_no CAS 双保险；advisory lock 保证锁序稳定无死锁 |
| 6 | Git 写入与 PG 状态竞争 | §12.4 状态×处置矩阵 + reconcile 幂等恢复；PG 侧一切状态迁移走账本状态机（单向触发器） |

## 17.3 锁序与隔离级别

- **标准锁序**（防死锁，全 PC 写路径统一）：`project advisory lock → proposal → approval → cognition_object/relationship（按 UUID 升序逐行）→ revision INSERT → cognition_version → version_member → audit_log → integration.outbox_event`。禁止逆序获取（V1.2/F-⑤：尾项追加 outbox——发布事务 ⑫b 同事务回写事件）。
- 隔离级别：Read Committed（全仓现状）+ 显式行锁；发布事务依赖 advisory lock 保证读到的版本号在事务内不再变化，不依赖 SERIALIZABLE。
- `ensure_governance_authorized` 为无锁判定（H-11 事实）：授权撤销与审批裁决的竞态窗口 ≤ 撤销事务提交时刻；审批行为终态事实，若裁决发生在授权生效窗口内则视为有效（V1 同权规则下可接受，写入 §23 风险跟踪）。

---

# 18. 版本发布事务（任务书 §14.1 十四步）

## 18.1 完整事务伪代码（保障层逐条标注）

```text
publish_cognition_version(project_id, proposal_id, operation_id):
  BEGIN;
    ①  pg_advisory_xact_lock(hashtext('pc_publish:' || project_id))        [DB·advisory 锁，§17.2-5]
        -- 写幂等：INSERT pc.write_operation(command='pc_publish_version', pending,
        --    UNIQUE(tenant_id,project_id,operation_id))；冲突且 completed → 返回既有版本（幂等）  [DB·唯一约束]
    ②  current := SELECT max(version_no) FROM pc.cognition_version WHERE project;
        IF proposal.expected_version_no <> COALESCE(current, 0) THEN
            proposal → failed; COMMIT; RETURN CAS_MISMATCH;                     [服务层·CAS]
    ②b 成员完整性断言〔V1.1（R5/F-05）〕：
        members := SELECT * FROM pc.proposal_member WHERE proposal_id AND
                   （对象成员：object_id AND object_revision_id 非空）
                OR （关系成员：relationship_id AND relationship_revision_id 非空）；
        IF members <> 全量成员集 THEN ROLLBACK;   -- 时序契约前提（§10.1 成员时序契约），
        -- 未齐备成员在此显式失败，而非流入步骤⑧触发互斥 CHECK 的隐晦 23514   [服务层·契约断言]
    ③  审批锚点主校验〔V1.1（R4/F-04）升级〕：
        approval := SELECT ... WHERE proposal_id AND status='approved' FOR UPDATE;
        recomputed := sha256(规范序列化(proposal 元数据 + members 有序集));   -- 服务层重算
        IF proposal.status <> 'approved' OR approval IS NULL
           OR recomputed <> approval.proposal_content_hash          -- 主校验：信任不可变的
                                                                    -- approval 锚定列
           OR approval.expected_version_no <> proposal.expected_version_no THEN
            ROLLBACK;   -- 不信任 proposal 侧可变列（其已由 trg_pc_proposal_anchors
                        -- 锚定，此处重算是第二道防线）                    [DB·行锁 + 服务层·重算]
    ④  IF NOT ensure_governance_authorized(project, actor) THEN ROLLBACK;       [服务层·A3 闸]
    ⑤  （审批行锁与锚点核验已并入步骤③）
    ⑥  IF EXISTS (SELECT 1 FROM pc.conflict c JOIN pc.conflict_object co
                  ON c.id = co.conflict_id
                  WHERE c.status IN ('detected','under_review','awaiting_human','reopened')
                    AND (co.object_id IN (提案目标对象集)
                         OR co.relationship_id IN (提案目标关系集)))
          THEN ROLLBACK;                                                        [服务层·L3 冻结校验]
        -- 〔V1.1（R6/F-06）〕同一校验亦为 approve 服务的前置步骤（双落点，对齐
        -- 01A §6.5「冻结确认入口」）；发布开始后新登记的 Conflict 不回溯阻断
        -- 本次发布（其生效面向下一版本），该语义在此显式声明。
    ⑦  snapshot_hash := sha256(规范序列化(全部成员 (kind,id,revision_id) 按
        kind,id,revision_id 字典序));   -- 〔V1.1（R2/F-02）〕先算后插：成员集在
        -- 版本行插入前已确定（步骤②b/③ 持有），一次性 INSERT 含最终 hash——
        -- 消除「占位回填」与 forbid_version_mutation 锚定触发器的矛盾
    ⑧  INSERT pc.cognition_version(version_no = current+1, source_proposal_id,
          confirmed_by=actor, snapshot_hash=⑦结果, predecessor_version_id=current);
        -- UNIQUE(tenant_id,project_id,version_no) 冲突 → CAS 失败路径          [DB·唯一约束]
    ⑨  INSERT pc.version_member（对象成员集 + 关系成员集，双维度）；
        -- 四复合 FK + 互斥 CHECK + 部分唯一索引逐行生效                        [DB·约束]
    ⑩  执行 §8.3 闭合断言 SQL；结果非空 → ROLLBACK（DEFERRED 触发器在 COMMIT 前兜底）；
        断言 hash(members) = ⑦的 snapshot_hash（一致性复核，防服务层计算缺陷） [DB·断言+触发器+服务层]
    ⑪  （步骤⑧⑨⑩ 完成成员固定——发布即原子，无 draft 态）
    ⑫  INSERT governance.audit_log(source_kind='pc_version.publish',
        target_type='pc_cognition_version', target_id=version.id,
        after_data={version_no, snapshot_hash, member_count, proposal_id},
        source_ref=operation_id);                                              [服务层·审计]
    ⑫b INSERT integration.outbox_event(pc.cognition.version_published,        [服务层·同事务事件回写，V1.2/F-⑤]
        payload={version_ref, version_no, snapshot_hash, proposal_ref});
        -- additive 追加：与业务行同事务提交（Transactional Outbox 语义），消费者只见提交后事实；
        -- 锁序尾项（§17.3）。发布失败走 ON ERROR 分支：仅审计（pc_version.publish_failed），不入事件目录。
    ⑬  UPDATE pc.proposal SET status='applied' WHERE id AND status='approved';
        -- 影响行数=0 → ROLLBACK（状态单向触发器已限 approved→applied）          [DB·触发器]
        UPDATE pc.write_operation SET status='completed', version_id=...;
    ⑭  COMMIT;
  ON ERROR: proposal → failed（独立事务），审计 pc_version.publish_failed（仅审计，不入事件目录）；原事务 ROLLBACK
```

## 18.2 数据库约束 vs 领域服务分工

| 步骤 | DB 约束保障 | 服务层保障 |
|---|---|---|
| ①② 幂等与 CAS | write_operation 唯一键；version_no 唯一键 | advisory lock、失败语义与重新分析 |
| ②b③⑤⑥⑩ 审批与冲突锚点 | approval 锚定列 immutable、单向状态、FOR UPDATE、proposal 内容锚定（V1.1） | 成员完整性断言（②b）、重算 hash 主校验（③，V1.1）、A3 权限闸、L3 冻结集合判定（approve+publish 双落点，V1.1）、snapshot_hash 先算后插 + 一致性复核（⑦⑩，V1.1） |
| ⑦⑧⑨ 成员固定 | version/member 全部 UNIQUE/CHECK/复合 FK + DEFERRED 闭合触发器 | 闭合断言 SQL 执行与可读错误、跨域端点通道校验 |
| ⑫⑬ 审计与收尾 | proposal 状态单向触发器 | audit_log 词表与 after_data 组装 |

原则：**凡是能用库层约束表达的，一律库层**（任务书推进原则 2）；服务层只承担「集合计算、权限判定、外部系统（Git/审计词表）交互」。

---

# 19. 审计设计

## 19.1 审计词表（source_kind = 'pc' 前缀点号式，沿 S02D-23 惯例）

| 动作 | action（source_kind='pc_*'） | before/after 摘要 |
|---|---|---|
| 对象创建/修订/废弃/替代 | `pc_object.create` / `pc_object.revise` / `pc_object.deprecate` / `pc_object.supersede` | before={lifecycle,current_revision} after={...} |
| 关系创建/修订/废弃/替代 | `pc_relationship.create/revise/deprecate/supersede` | 同上（含端点摘要） |
| 提案生命周期 | `pc_proposal.create/submit/withdraw/expire` | status 迁移 |
| 审批 | `pc_proposal.approve` / `pc_proposal.reject` | {approval_id, hash, expected_version_no, reason} |
| 版本发布 | `pc_version.publish` / `pc_version.publish_failed` | {version_no, snapshot_hash, member_count} |
| 冲突 | `pc_conflict.detect/resolve/dismiss/reopen` | status 迁移 + 裁决摘要 |
| 写恢复 | `pc_write.reconcile` / `pc_write.drift` | {operation_id, 状态迁移} |

## 19.2 字段映射（复用 governance.audit_log，0003:188-202 + 0004 + 0018）

| audit_log 列 | PC 填充 |
|---|---|
| source_kind / source_ref（0004:7-9） | `pc_*` / operation_id 或 proposal_id |
| actor_id + identity 三列（0018:25-28） | 审计 actor（真人主体；AI 动作 actor = 发起提案的 principal，Q2-C 惯例） |
| action / target_type / target_id | §19.1 词表 / 'pc_cognition_object' 等 / 对象 UUID |
| before_data / after_data | 状态摘要 JSONB（不含正文——正文权威在 Git，仅记 hash） |
| compensation_command | 发布失败补偿语义（proposal→failed）预留 |

## 19.3 不可覆盖保障

- audit_log 无 UPDATE/DELETE 路径（既有表无修改通道，0003 建表后仅追加列的迁移历史；PC 不引入任何 UPDATE audit 的代码路径）。
- 全部正式确认/审批/拒绝/发布/废弃/替代/冲突冻结解除动作**必须**在原事务内写审计（§18.1 ⑫⑬ 同事务；撤销授权等治理动作沿 approver 词表既有惯例，0040 头注）。
- 审计缺失 = 实现缺陷：§20 测试 T-24 类（发布成功但无审计行 → 断言失败）在 01C 验收清单内（01A §7.3）。

---

# 20. 数据库完整性测试方案（设计级，01C 实施）

> 执行载体：01C 集成测试（cargo test --workspace，测试库逐用例建数据）。错误码：23503=FK violation，23514=CHECK violation，23505=unique violation，P0001=触发器 RAISE，应用层 409/403=服务校验。

| # | 负向场景 | 前置数据 | 操作 | 预期 | 触发约束/校验 | 错误 |
|---|---|---|---|---|---|---|
| T-01 | Object Revision 指向其他 Object | 项目 P1 对象 A、P2 对象 B（同租户） | INSERT Revision(object_id=A, document_ref=B 的文档) | 拒绝 | FK `(tenant,project,object_id)`；文档项目级 FK 拦截（C-2，拦截面增强：document_revision_id ∈ document_id） | 23503 |
| T-02 | Relationship Revision 指向其他 Relationship | P1 关系 R、P2 关系 R2 | INSERT RR(relationship_id=R 的行、revision 链错配) | 拒绝 | FK `(tenant,project,relationship_id)` 或 revision 唯一锚 | 23503 |
| T-03 | Member 同时填写两种成员类型 | 已发布 Vₙ | INSERT member(object 与 relationship 四列全非空) | 拒绝 | 互斥 CHECK（§7.3） | 23514 |
| T-04 | Member 两种成员类型都为空 | 同上 | INSERT member(四列全空) | 拒绝 | 互斥 CHECK | 23514 |
| T-05 | Member 引用不存在的 Revision | 已发布 Vₙ | INSERT member(object_revision_id=随机 UUID) | 拒绝 | 复合 FK | 23503 |
| T-06 | Member 跨项目引用 | P1 版本 Vₙ；P2 对象 A2 | INSERT member(version=P1Vₙ, object=P2A2) | 拒绝 | 复合 FK（含 project_id） | 23503 |
| T-07 | 同一 Version 重复添加同一成员 | 已发布 Vₙ | 重复 INSERT 对象成员 (A, A-2) | 拒绝 | `uq_pc_vm_object` 部分唯一 | 23505 |
| T-08 | 正式版本发布后修改 Member | 已发布 Vₙ | UPDATE/DELETE member 行 | 拒绝 | `trg_pc_vm_immutable` | P0001 |
| T-09 | 关系 Revision 的 Source 端点对象成员未进版本 | 构造 R-3(source=A/A-2, target=B/B-5)；Vₙ₊₁ 成员含 (R,R-3) 与 (B,B-5)，缺 (A,A-2) | 发布事务 | 提交前拒绝 | 步骤⑨断言 + `trg_pc_vm_closure`（DEFERRED） | P0001 / 服务层错误清单 |
| T-10 | 同上但缺失 Target 端点 | 对称构造 | 发布事务 | 提交前拒绝 | 同上 | P0001 |
| T-11 | 端点 Revision 与 Member Revision 不一致 | Vₙ 含 (A,A-2) 与 (R,R-3)；R-3 端点 = A/A-9（A-9 未进版本） | 发布事务 | 拒绝 | 端点闭合断言（不一致必然表现为端点缺失，§8.4） | P0001 |
| T-12 | Snapshot Hash 与成员内容不一致 | 手工篡改 version.snapshot_hash | UPDATE version | 拒绝 | `trg_pc_version_immutable` 锚定列；正常路径 hash 由步骤⑩ CAS 复核 | P0001 |
| T-13 | 审批对象在审批后被替换 | 已 approved 审批行 | UPDATE approval.proposal_content_hash / proposal_id | 拒绝 | 锚定列 once-written 触发器 | P0001 |
| T-14 | 重复提交同一审批 | 已存在 pending 审批 | INSERT 第二条 pending | 拒绝 | `uq_pc_approval_pending` | 23505 |
| T-15 | 拒绝后的审批事实被覆盖 | 已 rejected 审批行 | UPDATE status='approved' 或改 decided_by | 拒绝 | 单向状态触发器 + 锚定列 | P0001 |
| T-16 | L3 Conflict 未解决时确认 L1/L2 变更 | conflict(under_review) 圈定对象 A；提案目标含 A | 调用发布事务 | 拒绝 | 步骤⑥ 冻结校验 | 服务层 409 |
| T-17 | 普通成员绕过权限检查 | 非成员/普通成员 principal | 直接调用审批/发布服务 | 拒绝 | `ensure_governance_authorized` | 403 |
| T-18 | 跨租户引用 | T1 对象；T2 上下文写入 | 任意 PC 内 FK 写入（member/endpoint/evidence） | 拒绝 | 全部复合 FK 首列 tenant_id | 23503 |
| T-19 | 跨项目引用 | 同租户 P1/P2 | 同 T-18（project 列维度） | 拒绝 | 复合 FK project 列 | 23503 |
| T-20 | Git-PG 重试产生重复 Revision | 账本 git_committed 行 | 重放步骤④ | 幂等：产出既有 Revision，不产生第二行 | operation_id 幂等 + revision 唯一约束 | 无错误（幂等返回） |
| T-21 | 相同 Operation ID 重复写入 | 已 completed 账本行 | INSERT write_operation 同键 | 幂等返回既有产出 | `UNIQUE(tenant_id,project_id,operation_id)` | 23505→捕获转幂等 |
| T-22 | 并发版本发布版本号冲突 | 两个可发布提案（同 expected_version_no） | 并发执行发布事务 | 一成一败（败者 proposal→failed，可重新分析） | advisory lock + `UNIQUE(tenant,project,version_no)` + CAS | 无 DB 错误；服务层 CAS_MISMATCH |
| T-23 | 正式版本被删除或修改 | 已发布 Vₙ | DELETE version / UPDATE 锚定列 | 拒绝 | FK 依赖（member 引用）+ 锚定触发器 | 23503 / P0001 |

附加断言类（正向验收）：T-24 发布成功后 audit_log 必含 pc_version.publish 行；T-25 版本还原测试（按 Vₙ 读回成员+Revision+Git 正文，四问逐条断言）；T-26 闭合触发器绕行测试（绕过服务直连 SQL 插入不闭合 Member → COMMIT 时 P0001）。

V1.1 增补（评审 R3/R4/R5 配套）：**T-27** RR 端点身份不一致拒绝（前置：关系 R 端点 A→B；直连 INSERT RR(source=C,C-1) → `trg_pc_rel_revision_endpoint_match` P0001）；**T-28** proposal 锚定拒绝（submitted 后 UPDATE proposal.proposal_content_hash → `trg_pc_proposal_anchors` P0001；发布步骤③重算 hash 与 approval 锚定不符 → ROLLBACK）；**T-29** 端点成员自动并入（create_relationship 提案构建后成员集自动含端点对象成员且 proposal_content_hash 覆盖之；发布成功且闭合断言通过）。

V1.2 增补（差异复核 C-4 配套）：**T-30** kb_document 通道四元组完整性（pc.source source_kind='kb_document' 但 target_domain≠'knowledge_base' 或 target_revision_id 为空 → 判别器/CHECK 拒绝 23514）；**T-31** 跨域引用对账（对账任务发现 KB 侧文档漂移 → pc_write.drift 审计 + revision_conflict 冲突登记，服务层测试；数据源=K3/PK1 只读 API）。

---

# 21. H-13 / H-11 验证结论

## 21.1 H-13：sqlx 0.8.6 对 0044 同号双文件的行为（源码级验证完成）

**验证方法**：阅读本地 cargo registry 中实际依赖版本的源码（`sqlx-core-0.8.6`、`sqlx-postgres-0.8.6`；Cargo.toml:38 `sqlx = "0.8"`）+ 迁移目录/账本/启动代码核对。未连接任何数据库（遵守本阶段禁止事项）。

| # | 证据 | 结论 |
|---|---|---|
| 1 | sqlx-core 0.8.6 `source.rs:57-144`（resolve_blocking） | 逐文件解析 `<VERSION>_<DESC>.sql`，version = 文件名前缀 i64；**无重复版本检测**；同号文件各自成为独立 Migration |
| 2 | `source.rs:142`（`sort_by_key(m.version)`，Rust 稳定排序） | 同号两文件相对顺序 = `read_dir` 目录序（**OS 决定，非字典序保证**——修正 Gate 0 B.2「按字典序执行」的表述：字典序仅在 OS 目录序恰为字典序时成立） |
| 3 | sqlx-postgres 0.8.6 `migrate.rs:119-120` | `_sqlx_migrations.version BIGINT PRIMARY KEY`——**每版本仅一行记账** |
| 4 | sqlx-postgres `migrate.rs:275-298`（execute_migration）+ core `migrator.rs:263-265` | 先执行迁移 SQL、后 INSERT 记账行、同事务提交；记账 INSERT 冲突 = 整个迁移失败回滚 |
| 5 | core `migrator.rs:163-183`（run_direct 主循环） | `applied_migrations` 为启动时快照 HashMap（version 为 key）；同号第二个文件命中同一记账行 → checksum 比对 |
| 6 | `apps/pm-api/src/main.rs:28` | `sqlx::migrate!("../../migrations").run(&pool).await?` ——服务启动即执行，错误直接中止启动 |
| 7 | 迁移目录实况 | `0044_s02_i3_source_traceability.sql` 与 `0044_team_onboarding.sql` 并存（内容不同 → checksum 必不同） |

**推演结论（必然性故障，非概率风险）**：

- **全新空库**：第一个 44（目录序在先者）apply 成功（记账 version=44/checksum_A）；第二个 44 → SQL 执行后 INSERT 记账 → 主键冲突 → 迁移失败回滚，其对象永不创建，服务启动失败。
- **已有 version=44 记账行的库**：两个 44 均命中同一记账行，checksum 至多一个匹配 → 另一个必抛 `MigrateError::VersionMismatch(44)` → 服务启动失败。
- 即：**只要目录中两个 44 并存，任何带 44 记账的库在下一次 pm-api 启动时必然失败；全新初始化也必然失败**。当前系统若仍在运行，说明其运行库尚未在两文件并存状态下重启过（或账本「已应用」行属于尚未覆盖该场景的库）〔待验证：运行库 `_sqlx_migrations` 中 version=44 的 checksum 归属〕。

**对起号方案的影响**：

1. **0045 起号结论成立**（源码级证据充分，Gate 0 的「验证前不得确认」条件已满足验证部分）：PC 新迁移从 0045 开始，禁止再产生重复号。
2. **0044 修复（01C 前置项，本阶段不执行；〔V1.1〕2026-11-07 用户已授权修复方案）**：连库查 `_sqlx_migrations WHERE version=44` 的 checksum → 与两个文件的实际 checksum 比对 → 保留记账所属文件为 44，另一文件重命名顺移为 0045+（内容不变，version 随文件名变化）；若目标环境为全新库则直接重命名其一即可。同步回填迁移账本（0044_s02_i3 漏记项）。
3. **账本勘误（01C 一并处理）**：`docs/ai-pm-migration-ledger.md:14` 仅登记 `0044_team_onboarding`，未登记 `0044_s02_i3_source_traceability.sql`（commit 5936105 先合入）——账本与目录不一致，按账本规则 2「不得改已应用迁移」原则修复时需同时回填账本。

## 21.2 H-11：现有审批链 FOR UPDATE 使用完整性

| 核查点 | 事实 | 证据 |
|---|---|---|
| 需求确认链锁 | 确认事务内对 requirement 行与 revision 行 `FOR UPDATE` | requirement.rs:251,270 |
| 审批授权闸 | `ensure_governance_authorized` 为无锁只读判定（Lead 查询 + approver_grant 查询），不在任何行锁保护内 | approver.rs:56-79 |
| 通用审批链 | `ai.change_proposal` / `governance.approval_request` 全链无行锁；1:1 靠 proposal_id UNIQUE；状态无 CHECK 无触发器 | grep 全仓 + 0003:157-168 |
| 锁序一致性 | 现有链无统一锁序文档；delivery/outbox 各自独立（outbox_handlers.rs:64 先锁 task） | 各文件 |
| 隔离级别 | 全仓 Read Committed（无 SET TRANSACTION 显式升级） | 代码检索 |

**对 PC 的影响与处理**（PC-D-01 选择 PC 专属结构后，approval_request 弱结构不再被 PC 复用，H-11 的修复责任随之转移）：

1. PC 审批/发布路径**不继承**上述缺陷：§10.2 触发器三件套 + §17.2 场景 4/5 行锁 + §17.3 统一锁序。
2. approval_request / change_proposal 的 m-3 加固归 S01 既有链路自身演进（非 PC 前置，01A §6.5 前提随 PC-D-01 裁决解除）。
3. `ensure_governance_authorized` 复用时保留已知竞态窗口（授权撤销 vs 审批提交），V1 接受并跟踪（§23 R-3）。

---

# 22. PC-01C 实施建议

## 22.1 实施顺序建议

```text
1. Migration（0045）：schema pc + 17 表（含 pc.document/pc.document_revision，C-1）+ 触发器 + 索引 + 类型种子
2. pm-domain：pc 实体行 struct（FromRow）
3. pc-domain service crate：
   a. 类型注册加载与启动校验（代码注册表 ⊆ 字典表）
   b. Object/Revision 服务（推号、CAS、Git-PG 协议）
   c. Relationship/Relationship Revision 服务（端点校验、锚定）
   d. Proposal 服务（成员集、hash 计算、状态机）
   e. Approval 服务（FOR UPDATE、A3 闸、审计）
   f. 发布服务（§18.1 十四步 + 闭合断言 + snapshot_hash）
   g. Conflict 服务（冻结判定）
   h. reconcile 后台任务（§12.5）
4. API 层（handler 只调服务，禁直写——负向测试证明）
```

## 22.2 Migration 要点

1. 起号 0045（§21.1）；实施前先完成 0044 修复（§21.1 结论 2，**已获授权**，执行归 01C）。
2. 单迁移收编 vs 四段拆分（注册表/六核心/治理/追溯）：推荐**单迁移**（表间 FK 依赖密集，拆分只增加中间状态无收益）；01C 定稿。
3. 种子 INSERT 与 DDL 同迁移（object_type 14 行 + relationship_type 17 行，§15.5）；种子变更走新迁移（不做幂等 ON CONFLICT 掩盖重复）。
4. **pc.document/pc.document_revision 建表同迁移执行**〔V1.2/C-1/C-3：替换原 R9 doc_type ALTER 项〕：正文承载两表建表（DDL 见服务化架构 V1.1 §8.2；双键隔离模式 §14.2），随 0045 落地；**原「ai.project_document doc_type 扩值 'pc_object'」ALTER 项取消**（ai 表不再承载 PC 正文，评审 C-3 排查无遗漏依赖）。
5. **对象创建顺序**〔V1.1（F-13 注记）+V1.2 补〕：公共触发器函数（§15.3）最先创建；**pc.document → pc.document_revision 先于 pc.cognition_revision**（后者 FK 反向依赖文档两表；文档两表间为单向 FK，无循环，无需两步法）；object↔revision、relationship↔relationship_revision 循环 FK 采用「建表（不含自引用 FK）→ ALTER TABLE ADD CONSTRAINT」两步；种子 INSERT 在全部约束就位后执行。
6. 全程不含任何历史业务数据迁移/回填/转换（零迁移硬边界）。

## 22.3 服务实现要点

1. 安全闸唯一写入口（API handler 禁直写正式表）；正式认知写入前 M3 门禁（台账 M3 里程碑）。
2. 锁序（§17.3）作为代码评审检查项。
3. Git 提交 message 统一 trailer：`PC-Operation-Id: <operation_id>`（reconcile 反查依据）。
4. snapshot_hash 规范序列化函数须确定性（成员排序规则固定），01C 以属性测试锁定。

## 22.4 台账对接

- 台账唯一性：本阶段交付对应台账 **PC-0006**（语义已由 01A §7.8 替换为「PC 新表 Migration 起号规划（0045+）与设计」）——台账状态更新见本文档配套的台账修订（docs/product-cognition-knowledge-base/product-cognition-development-plan-v1.md）。
- PC-0006 完成后，阶段 1（I1）/阶段 3（I3）即 PC-01C 具备开工条件（前置 = 本文档终审 + Migration 实施）。