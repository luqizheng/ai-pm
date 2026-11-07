# PC-01B — 版本层与端点闭合（V1.1 拆分件 03，原文档 §7~§8）

> **拆分声明**：本文件为《PC-01B 数据模型与数据库设计 V1.1》拆分件之一，内容为原文档 §7~§8，**零设计变更**；章节编号保持原文档编号。总纲/裁决/验收见拆分件 00；身份层表结构见拆分件 02。

---

# 7. Cognition Version / 双维度 Member 模型

## 7.1 PC-D-03 设计裁决：完整快照（唯一推荐）

| 方案 | 还原四问能力 | 端点闭合可约束性 | diff 查询 | 存储成本 | 结论 |
|---|---|---|---|---|---|
| **A 完整快照** | 单表 Member 全量可得（§7.3） | 发布事务 + DEFERRED 触发器直接在成员集上断言（§8） | 两版本 Member 集合差集，单 SQL | 每版本 ≈ 活跃对象数 + 关系数行（项目级数量级 10²~10⁴ 行，PG 无压力） | **推荐** |
| B 增量快照 | 需沿 predecessor 链回放合并，还原逻辑落在应用层 | 闭合检查需先重放出完整成员集才能断言——**库层无法在行/集上直接表达** | 需重放 | 省 | 否决 |
| C 增量 + 基线合并 | 同 B 且多基线分支 | 更弱 | 需合并算法 | 省 | 否决 |

理由：〔裁决〕正式版本必须完整还原四问（01A §6.2）+ 库层闭合可约束性（任务书推进原则 1「先解决版本完整性，再谈表结构」）+ 审计可独立验证。存储成本与正确性风险相比可忽略。

## 7.2 pc.cognition_version — 正式版本头（不可变）

```sql
CREATE TABLE pc.cognition_version (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id              UUID NOT NULL REFERENCES core.tenant(id),
    project_id             UUID NOT NULL,
    version_no             BIGINT NOT NULL CHECK (version_no >= 1),
    status                 TEXT NOT NULL DEFAULT 'published' CHECK (status IN ('published','archived')),
    confirmed_by           UUID NOT NULL REFERENCES core.principal(id),
    confirmed_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    source_proposal_id     UUID NOT NULL,
    snapshot_hash          TEXT NOT NULL,        -- 双维度成员集合的规范哈希（§18.1 步骤⑩）
    predecessor_version_id UUID,                 -- 版本演进链（V1 首版本 NULL）
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    UNIQUE (tenant_id, project_id, version_no),  -- 版本号并发闸（CAS 配套，§17.2 场景 5）
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, source_proposal_id)
        REFERENCES pc.proposal (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, predecessor_version_id)
        REFERENCES pc.cognition_version (tenant_id, project_id, id)
);

CREATE TRIGGER trg_pc_version_immutable BEFORE UPDATE OR DELETE ON pc.cognition_version
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_version_mutation();
-- pc.forbid_version_mutation：锚定列（version_no/confirmed_by/confirmed_at/source_proposal_id/
-- snapshot_hash/predecessor_version_id/created_at）once-written；status 仅 published→archived 单向。
-- （范式 = 0026:273-296，全文见 §15.5）
```

## 7.3 pc.version_member — 双维度成员（本阶段最高优先级表）

```sql
CREATE TABLE pc.version_member (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                 UUID NOT NULL REFERENCES core.tenant(id),
    project_id                UUID NOT NULL,
    version_id                UUID NOT NULL,
    -- ── 对象成员维度 ────────────────────────────────────────────
    object_id                 UUID,
    object_revision_id        UUID,
    -- ── 关系成员维度 ────────────────────────────────────────────
    relationship_id           UUID,
    relationship_revision_id  UUID,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, version_id)
        REFERENCES pc.cognition_version (tenant_id, project_id, id),
    -- 四复合 FK（MATCH SIMPLE：参与列任一为 NULL 则该约束跳过——互斥 CHECK 保证恰一组齐全）
    FOREIGN KEY (tenant_id, project_id, object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, object_id, object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id, relationship_revision_id)
        REFERENCES pc.relationship_revision (tenant_id, project_id, relationship_id, id),
    -- 严格二选一（任务书 §4.3）：不得两组同时为空或同时非空
    CHECK (
        (    object_id IS NOT NULL AND object_revision_id IS NOT NULL
         AND relationship_id IS NULL AND relationship_revision_id IS NULL )
        OR
        (    relationship_id IS NOT NULL AND relationship_revision_id IS NOT NULL
         AND object_id IS NULL AND object_revision_id IS NULL )
    )
);

-- 同版本不重复收录同一成员（两把部分唯一索引）
CREATE UNIQUE INDEX uq_pc_vm_object
    ON pc.version_member (tenant_id, project_id, version_id, object_id, object_revision_id)
    WHERE object_id IS NOT NULL;
CREATE UNIQUE INDEX uq_pc_vm_relationship
    ON pc.version_member (tenant_id, project_id, version_id, relationship_id, relationship_revision_id)
    WHERE relationship_id IS NOT NULL;

CREATE INDEX idx_pc_vm_version  ON pc.version_member (tenant_id, project_id, version_id);
CREATE INDEX idx_pc_vm_object   ON pc.version_member (tenant_id, project_id, object_id, object_revision_id);
CREATE INDEX idx_pc_vm_rel      ON pc.version_member (tenant_id, project_id, relationship_id, relationship_revision_id);

-- 发布后成员不可变：全量禁 UPDATE/DELETE（版本创建即发布，§4.2）
CREATE TRIGGER trg_pc_vm_immutable BEFORE UPDATE OR DELETE ON pc.version_member
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();
```

设计说明：

1. **互斥 CHECK + 四复合 FK 的配合**：互斥 CHECK 保证「恰一组（对象或关系）两列齐全」；MATCH SIMPLE 使 NULL 组的 FK 不参与校验，齐全组的复合 FK 完整校验「成员 Revision 属于对应 Object/Relationship 且同租户同项目」（任务书 §4.3 要求 1/2/4 一步落实，修复 M-8 型缺陷）。
2. 「同版本不重复收录」由两把部分唯一索引承担（要求 3）；「不遗漏」由 §8 闭合断言承担。
3. 要求 5/6（发布后 Member 不可修改、不得以删除/更新改写历史版本）：`trg_pc_vm_immutable` 全行禁改 + 版本头锚定列不可变；Member 没有任何合法 UPDATE 场景（无状态列）。
4. 版本 diff：`(SELECT ... FROM v_n EXCEPT SELECT ... FROM v_m)` 对称差即双维度变更集（对象与关系各自独立计算，§16 Q10）。

---

# 8. Relationship 端点闭合设计（任务书 §4.4）

## 8.1 问题定义

已发布的 Cognition Version V 包含 Relationship Revision R-3（Source = Object A / A-2，Target = Object B / B-5）时，V 必须同时包含对象成员 (A, A-2) 与 (B, B-5)。遗漏端点 = 版本不可还原关系语义。该约束是**集合级**（涉及同 version_id 下的多行成员 + 跨表读 Revision 端点），无法用任何静态 FOREIGN KEY 表达，必须组合三种机制。

## 8.2 三层防线（库层优先，服务层兜底为可读错误）

| 层 | 机制 | 拦截的缺陷 | 时机 |
|---|---|---|---|
| 1 | Relationship Revision 端点四复合 FK（§6.4） | 端点 Revision 不属于端点 Object、跨项目引用 | 任何写入（含绕闸直写） |
| 2 | 发布事务 SQL 断言（§18.1 步骤⑨） + 提交前 DEFERRED 约束触发器（§8.3） | 成员遗漏端点、端点 Revision 与成员 Revision 不匹配 | 发布事务内、COMMIT 前 |
| 3 | 服务层二次校验（发布服务在事务内重查成员集与端点闭合，产出可读错误清单） | 同上（人可读诊断）；同时校验跨域端点引用通道状态 | 发布事务内 |

## 8.3 闭合检查 SQL 与约束触发器

**发布事务内断言（步骤⑨，错误即 ROLLBACK）**：

```sql
-- 闭合断言：本版本每个关系成员的 PC 内端点 (object_id, object_revision_id)
-- 必须同时是本版本的对象成员。跨域端点（*_object_id IS NULL）跳过。
SELECT rr.relationship_id, rr.revision_no,
       CASE WHEN rr.source_object_id IS NOT NULL THEN 'source' ELSE 'target' END AS broken_side
FROM pc.version_member vm
JOIN pc.relationship_revision rr
  ON (rr.tenant_id, rr.project_id, rr.relationship_id, rr.id)
   = (vm.tenant_id, vm.project_id, vm.relationship_id, vm.relationship_revision_id)
WHERE vm.tenant_id = :tenant_id AND vm.project_id = :project_id
  AND vm.version_id = :version_id
  AND (
        (rr.source_object_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM pc.version_member m2
            WHERE m2.tenant_id = vm.tenant_id AND m2.project_id = vm.project_id
              AND m2.version_id = vm.version_id
              AND m2.object_id = rr.source_object_id
              AND m2.object_revision_id = rr.source_object_revision_id))
     OR (rr.target_object_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM pc.version_member m2
            WHERE m2.tenant_id = vm.tenant_id AND m2.project_id = vm.project_id
              AND m2.version_id = vm.version_id
              AND m2.object_id = rr.target_object_id
              AND m2.object_revision_id = rr.target_object_revision_id))
      );
-- 结果集非空 → ROLLBACK，错误消息携带 broken 成员与缺失端点
```

**约束触发器（库层最后防线，防绕过服务层直写 Member）**：

```sql
CREATE FUNCTION pc.enforce_member_endpoint_closure() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    broken INT;
    v_tenant UUID; v_project UUID; v_version UUID;
BEGIN
    v_tenant  := COALESCE(NEW.tenant_id,  OLD.tenant_id);
    v_project := COALESCE(NEW.project_id, OLD.project_id);
    v_version := COALESCE(NEW.version_id, OLD.version_id);

    SELECT count(*) INTO broken
    FROM pc.version_member vm
    JOIN pc.relationship_revision rr
      ON (rr.tenant_id, rr.project_id, rr.relationship_id, rr.id)
       = (vm.tenant_id, vm.project_id, vm.relationship_id, vm.relationship_revision_id)
    WHERE (vm.tenant_id, vm.project_id, vm.version_id) = (v_tenant, v_project, v_version)
      AND ((rr.source_object_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM pc.version_member m2
                WHERE (m2.tenant_id, m2.project_id, m2.version_id)
                    = (v_tenant, v_project, v_version)
                  AND m2.object_id = rr.source_object_id
                  AND m2.object_revision_id = rr.source_object_revision_id))
        OR (rr.target_object_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM pc.version_member m2
                WHERE (m2.tenant_id, m2.project_id, m2.version_id)
                    = (v_tenant, v_project, v_version)
                  AND m2.object_id = rr.target_object_id
                  AND m2.object_revision_id = rr.target_object_revision_id)));

    IF broken > 0 THEN
        RAISE EXCEPTION 'version % has % relationship member(s) with endpoints missing from the version', v_version, broken;
    END IF;
    RETURN NULL;   -- AFTER statement trigger
END $$;

CREATE CONSTRAINT TRIGGER trg_pc_vm_closure
    AFTER INSERT ON pc.version_member
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_member_endpoint_closure();
```

设计要点：

1. **DEFERRABLE INITIALLY DEFERRED**：发布事务按「对象成员 → 关系成员」顺序批量插入时，行级触发器不逐行报错，统一在 COMMIT 前对已插入集合校验——插入顺序无关性得到保证；同时任何绕过发布服务的事务（即使手工 SQL）也无法提交不闭合的版本。
2. 触发器仅挂 `INSERT`：Member 无 UPDATE/DELETE 路径（immutable 触发器先拦），无需重复覆盖。
3. 跨域端点跳过库层闭合（其 Revision 不在 PC 库内），由服务层按 §13.3 通道状态表校验并在发布审计中记录校验结果。
4. 触发器实现为逐行（FOR EACH ROW）但对整版本计数校验，代价 O(版本成员数 × 2)；版本成员数量级下可接受；如 01C 压测不达标可降级为 AFTER STATEMENT 语句级触发器（语义不变），本设计先按行级表达。

## 8.4 端点一致性总矩阵（任务书 §4.4 各问对应）

| 完整性问题 | 保障机制 |
|---|---|
| Relationship Revision 端点指向其他 Object / 跨项目 Revision | 端点四复合 FK（§6.4，防层 1） |
| Version Member 引用不存在的 Revision / 跨项目成员 | Member 四复合 FK（§7.3） |
| Version 遗漏关系端点对象成员 | 发布事务断言（§18.1 ⑨）+ DEFERRED 触发器（§8.3） |
| Member 关系 Revision 与端点 Revision 不匹配 | 端点快照在 Revision 内（自包含）+ 复合 FK 锚定：成员引用的 relationship_revision 本身携带端点 Revision，二者不可能不一致——不一致只可能表现为「端点对象成员缺失」，由闭合断言拦截 |
| 正式版本发布后端点漂移 | Revision 行 immutable（§5.3/§6.4）+ Relationship 端点锚定列 immutable（§6.5）+ Member immutable（§7.3） |
| 跨域端点 | 三元定位非空 CHECK + 服务层通道校验（§13.3），不伪造 FK |
| 历史版本还原语义 | 还原 = 读历史成员 + Revision 端点快照；全部行不可变 ⇒ 还原结果确定且稳定 |