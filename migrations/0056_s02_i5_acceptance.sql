-- 0056_s02_i5_acceptance: S02-I5 · Acceptance（需求级验收）。
--
-- 契约：S02-04 V1.2 §11（Q-11~Q-14 / S02D-09~12）。
--   Q-11/S02D-09  载体 = 新表锚 requirement_id（验收语义属需求级治理内容，
--                 不绑修订；established_in_revision_id 仅留痕）。
--                 硬约束：不可改造 delivery.task_acceptance_criterion（0015，
--                 硬锚 task + 7 处读写点）——本表独立，任务级验收链不动。
--   Q-12/S02D-10  验收项不随 Revision 版本机重版本；增删改 = 独立受控命令
--                 （真人 + Lead OR 显式 Approver，复用 0040 治理闸）+ 审计；
--                 确认时一致性提示（漂移查询）由服务面 criteria_drift_since 交付。
--   Q-13/S02D-11  承载层 = verification_kind 扩枚举（既有四值 + performance/
--                 security 验证族）+ params JSONB（指标/阈值/测量方法/审查准则/
--                 通过规则）；禁止按类型分表；形状校验归应用层（V1 宽松：
--                 仅要求 object，族内 required keys 待消费方定义后收紧）。
--   Q-14/S02D-12  零 Task NFR：需求级验收裁决入口 = 本表 verdict 列 +
--                 record_criterion_verdict 命令（不挂 Task 链，零 Task NFR
--                 不会永久 awaiting）；该接口即 S02 向 S04/S05 的声明面。
--
-- 命名/风格对齐 0045：tenant 复合 FK 对齐 0019/0045；审计走 append_audit
-- 点号式事件（requirement.criterion_*）；无触发器（审计在服务层同事务写）。

CREATE TABLE delivery.requirement_acceptance_criterion (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL,
    project_id    UUID NOT NULL,
    requirement_id UUID NOT NULL,
    position      INTEGER NOT NULL DEFAULT 0,
    statement     TEXT NOT NULL CHECK (length(btrim(statement)) > 0),
    verification_kind TEXT NOT NULL DEFAULT 'manual'
                  CHECK (verification_kind IN ('manual', 'automated_test', 'artifact_exists',
                                               'review', 'performance', 'security')),
    params        JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- 留痕：确立时需求所在正式版本（无正式版本 = NULL，Q-11「不绑修订」）
    established_in_revision_id UUID,
    -- Q-14：需求级验收裁决（不挂 Task 链；re-verdict 覆盖更新，审计留痕）
    verdict       TEXT NOT NULL DEFAULT 'pending'
                  CHECK (verdict IN ('pending', 'passed', 'failed')),
    verdict_note  TEXT,
    verdict_by    UUID,
    verdict_at    TIMESTAMPTZ,
    created_by    UUID NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id)
);

CREATE INDEX idx_req_acceptance_requirement
    ON delivery.requirement_acceptance_criterion (requirement_id, position, id);
