-- 0016_policy_composition: 完整策略合成所需的层级字段（§15.6）。
--
-- 背景：0013 只给 organization / project / task_type 三层加了
-- `require_human_approval`，能力类（auto_accept / auto_merge /
-- auto_publish_knowledge）与额外校验要求（additional_checks）只有任务级
-- override 有存储位，导致四层合成在上三层缺字段、无法落地。本迁移只补列，
-- 不改变 0013 的审批语义（`require_human_approval` 仍为 NOT NULL，缺行＝该层不表态）。
--
-- 能力类三列可空：NULL 表示该层**不表态**（不额外放宽、也不额外收紧），
-- 与 §15.6「任务覆盖为 None 表示继承」的口径一致。
-- additional_checks 为「只能增加」的约束类字段：空数组＝该层不追加要求。

ALTER TABLE delivery.policy_scope
    ADD COLUMN IF NOT EXISTS auto_accept BOOLEAN,
    ADD COLUMN IF NOT EXISTS auto_merge BOOLEAN,
    ADD COLUMN IF NOT EXISTS auto_publish_knowledge BOOLEAN,
    ADD COLUMN IF NOT EXISTS additional_checks JSONB NOT NULL DEFAULT '[]';

-- 不依赖隐式约束名（R2）：按 pg_constraint 元数据判断后补 CHECK。
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM pg_constraint
         WHERE conrelid = 'delivery.policy_scope'::regclass
           AND conname = 'policy_scope_additional_checks_array'
    ) THEN
        ALTER TABLE delivery.policy_scope
            ADD CONSTRAINT policy_scope_additional_checks_array
            CHECK (jsonb_typeof(additional_checks) = 'array');
    END IF;
END $$;