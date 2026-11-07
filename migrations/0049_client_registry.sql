-- 0049_client_registry: V-05-1 Client Registry 最小生产可用集
-- （设计 docs/project-ai-team-v2/ai-pm-v05-client-registry-design-v1.md V1.0；
--   联合架构裁决 2026-11-07：Q-A~Q-D 通过 + 工程约束 A/B/C。
--   实施基线 docs/project-ai-team-v2/ai-pm-v05-implementation-baseline-v1.md 批 1。）
--
-- 取号：台账预留 0049（现最大 0048_kb_pc_intake；落地前已重核 `ls migrations/`）。

-- ── 1) device 升级（0027 骨架扩展；凭据单列，轮换 = issue-new → revoke-old）─────
ALTER TABLE devenv.device
    ADD COLUMN credential_key_hash   TEXT,
    ADD COLUMN credential_issued_at  TIMESTAMPTZ,
    ADD COLUMN credential_revoked_at TIMESTAMPTZ,
    -- V1 自治权限面（Q-C 双轨先行）：签发时可裁剪；lmms IAM 字典落地后合并。
    ADD COLUMN granted_permissions   TEXT[] NOT NULL
        DEFAULT ARRAY['PM_MCP_READ','PM_MCP_AGENT_REPORT','PM_MCP_COMMAND'];

-- client_name（本表 name）租户内唯一归属（Q-D）
ALTER TABLE devenv.device
    ADD CONSTRAINT uq_device_tenant_name UNIQUE (tenant_id, name);
-- 约束 B 陪伴唯一约束（复合外键目标）
ALTER TABLE devenv.device
    ADD CONSTRAINT uq_device_tenant_id UNIQUE (tenant_id, id);

CREATE INDEX idx_device_credential_hash
    ON devenv.device (credential_key_hash) WHERE credential_key_hash IS NOT NULL;

-- ── 2) Run 的稳定 Client 身份（约束 B：租户一致性由数据库约束保障）────────────
ALTER TABLE delivery.task_run
    ADD COLUMN lease_client_id UUID;
ALTER TABLE delivery.task_run
    ADD CONSTRAINT fk_task_run_client_tenant
    FOREIGN KEY (tenant_id, lease_client_id)
    REFERENCES devenv.device (tenant_id, id);
CREATE INDEX idx_task_run_client
    ON delivery.task_run (lease_client_id) WHERE lease_client_id IS NOT NULL;

-- ── 3) 历史回填（三分类；裁决原则：历史兼容 ≠ 历史身份可信）──────────────────
-- 可靠对应（自动回填）= lease_owner LIKE 'client:%'（V-05-1 新格式；本迁移时点
-- 尚无此类行——规则先落以覆盖滚动升级窗口）：
UPDATE delivery.task_run tr
   SET lease_client_id = (substring(tr.lease_owner from 8))::uuid
  WHERE tr.lease_client_id IS NULL
    AND tr.lease_owner LIKE 'client:%'
    AND EXISTS (SELECT 1 FROM devenv.device d
                 WHERE d.id = (substring(tr.lease_owner from 8))::uuid
                   AND d.tenant_id = tr.tenant_id);
-- 仅自报（mcp:{client_name}:{token8}）→ 保持 NULL（裁决：自报文本不构成真实
-- 身份，不自动认定）；该谓词即 legacy 分类标记（lease_client_id IS NULL ∧
-- lease_owner LIKE 'mcp:%'），无行变更故无审计行。Worker/NULL → 无 Client 语义。
