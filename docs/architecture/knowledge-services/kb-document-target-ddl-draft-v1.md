# kb.document / kb.document_revision 目标形态设计稿（P1-4 交付物）

| 项 | 值 |
|---|---|
| 版本 | V1.0（2026-11-07，草案） |
| 性质 | **仅设计稿，不实施**。作为 ADR-06b「迁移工程六问」正式分析的输入；本文件不构成迁移批准 |
| 上游 | V1.1 §7.1（KB 行：目标 kb.document / kb.document_revision）、§8.1/§8.2（正文权威三分 + pc.document 模式）、§15.3（KB 侧对齐清单）；ADR-06a（容器三分家）/ ADR-06b（框架批准，批准≠批准迁移实施）/ ADR-09（Git-PG 一致性推荐 C） |
| 同构基线 | V1.1 §8.2 `pc.document` DDL 模式（评审 N-1 补齐后的双键范式）+ 0006 现 `ai.project_document` 语义（doc_key/doc_type/canonical_path/revision 四态）+ 0019 租户化范式 |
| 现状事实 | §4.7：`ai.project_document`/`ai.document_revision` 为跨领域共享容器（5 方使用）；KB 使用部分目标态迁 `kb.*`；迁移实施必须经 06b 六问单独批准 |

---

## 1. 目标定位

KB-Service 的文档身份与修订承载（KBGIT = 现 `AI_PM_KNOWLEDGE_ROOT` Git 根，正文权威）：

| 表 | 承载 | 说明 |
|---|---|---|
| `kb.document` | 文档身份 | doc_key / doc_type / canonical_path（KBGIT 内路径）；**不存正文**（0006 原则维持） |
| `kb.document_revision` | 修订 | revision_no / git_commit_sha（KBGIT commit，正文权威锚）/ content_hash / 生命周期状态 |

KB 业务专属五表（document_proposal / knowledge_reference / document_section_index / document_index_state / knowledge_write_operation）目标态同迁 `kb.*` 同名（§7.1）；本稿只给 document 双表 DDL——五表为纯改名迁址（同 schema 内表名不变更语义），06b 六问统一覆盖。

## 2. 目标 DDL（草案）

```sql
-- 06b 批准后随 KB 迁移实施；此处为目标形态声明（伪 DDL，实施时按当时范式校准）。
CREATE TABLE kb.document (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES core.tenant(id),
    project_id     UUID NOT NULL,
    doc_key        TEXT NOT NULL CHECK (doc_key ~ '^[A-Za-z0-9._-]+$'),
    doc_type       TEXT NOT NULL CHECK (doc_type IN
                       ('requirement', 'decision', 'architecture', 'knowledge')),
                       -- 值域沿 0006；PC 域值不入此表（pc.document 另立，C-1）
    canonical_path TEXT NOT NULL,               -- KBGIT 内路径
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),                             -- 0019 租户级身份键（组合 FK 目标锚）
    UNIQUE (tenant_id, project_id, id),                 -- 项目级身份键（评审 N-1 双键范式）
    UNIQUE (tenant_id, project_id, doc_key),
    UNIQUE (tenant_id, project_id, canonical_path),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE TABLE kb.document_revision (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES core.tenant(id),
    project_id     UUID NOT NULL,
    document_id    UUID NOT NULL,
    revision_no    INT NOT NULL CHECK (revision_no >= 1),
    git_commit_sha TEXT NOT NULL,               -- 必须指向 KBGIT（§8.3-5 同构约束）
    content_hash   TEXT NOT NULL,               -- sha256(markdown)，对账锚
    status         TEXT NOT NULL DEFAULT 'draft' CHECK (
                       status IN ('draft', 'published', 'rejected', 'rolled_back')),
                       -- 文档生命周期四态沿 0006（≠ write ledger 状态机，§3）
    change_summary TEXT,
    created_by     UUID NOT NULL REFERENCES core.principal(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),                 -- revision 级项目身份键（组合 FK 引用锚）
    UNIQUE (tenant_id, project_id, document_id, revision_no),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, document_id)
        REFERENCES kb.document (tenant_id, project_id, id)
);
CREATE INDEX idx_kb_document_revision_document
    ON kb.document_revision (tenant_id, document_id, revision_no DESC);

-- append-only：修订 = 新 commit + 新 Revision；回滚 = revert 新 commit（新行）。
-- 行级 UPDATE/DELETE 由触发器拒绝（同构 pc.forbid_row_update() 范式，PC-01B §15.3）。
```

与 §8.2 `pc.document` 的同构对照：双键范式（UNIQUE(tenant_id,id) + 项目级身份键）、组合 FK 锚、append-only 触发器全部一致；差异仅 doc_type 值域（KB 沿 0006 四值，无 'pc_object'——C-3 已取消 doc_type ALTER）。

## 3. ADR-09 协议对齐（Git-PG 一致性）

| 构件 | KB 侧目标形态 |
|---|---|
| 写幂等账本 | `kb.write_operation`（现 `ai.knowledge_write_operation` 迁址；唯一键 = 幂等键 + tenant） |
| 状态机 | `pending → git_committed → completed`，失败收敛 `failed` / `abandoned`（四态+终态，可判定性保证；§12 PC 侧同构） |
| 失败语义 | 业务事务失败 `failed` 用**独立连接**补写（同事务回滚会连带回滚标记）；`retry_count > MAX_AUTO_RETRY(5)`（账本层，与投递层 8 次独立计数，ADR-12）→ 人工介入 |
| commit trailer | KBGIT commit trailer 前缀 `KB-`（双根区分：KGIT=KB- / PCGIT=PC-）；trailer 反查 + 时间窗 + 路径过滤 |
| reconcile | 周期任务：`kb.write_operation` 待决行 → ① trailer 反查 commit → ② blob hash 对账 → ③ 补 Revision 落库；drift → `kb_write.drift` 审计 + 冲突登记 |
| 对账锚 | `kb.document_revision.content_hash` = KBGIT blob sha256 三方一致（账本 expected / 表值 / Git 实际） |

## 4. 迁移评估面（06b 六问的输入素材，非答案）

1. **数据搬运规模**：两表全量行复制（id 不变，保引用稳定）；FK 重挂（document_revision → kb.document）；五张 KB 专属表同批迁。
2. **引用方切换顺序**：`crates/pm-application/src/knowledge*.rs` 九文件 + `knowledge_write_coordinator` 写路径切 `kb.*`；跨域引用方（pc.source/pc.evidence 目标态、审计展示）按 §7.3 四元组解析，**引用不随之改写**（domain='knowledge_base' 不变，物理位置对引用方透明）。
3. **过渡期共享容器收尾**：Goal / Discovery / Task Contract 保留 `ai.document*`（G-1=a' 终态共址）；FK `0015:50`（task_contract → ai.project_document）不动；是否改名 `platform.document` 由 06b 一并裁决（不强制）。
4. **双写窗口**：零历史数据双写（红线）；切换 = 单次原子迁移 + 代码同版本切换，回滚预案 = 06b 六问正式分析项。
5. **DB Role**：迁移完成后 `kb_service` 对 `ai.document*` 的过渡期白名单收回（§16.4）。
6. **守卫联动**：迁移实施批次须同步更新 `guard_ai_container_users.rs` 白名单（KB 九文件移出、迁移清单补 06b 实施迁移号）。

## 5. 与本批次（Phase 1-3）的边界

- 本批次**不建** `kb.*` 表、**不改**任何 KB 写路径（Phase 4 = KB 独立化 + 本稿转正式 06b 分析）。
- 本稿落盘即满足 Phase 1 验收「kb.*/pc.document* 目标形态设计稿」；`pc.document*` 目标形态已在 V1.1 §8.2 正文承载，不重复。
