-- 0028_projc_object_key_unique.sql
--
-- P3-D04-02 A-1：projc.cognition_object 补具名身份唯一约束（依赖核查报告 S-06；
-- 《P3-D04-02 Step3 启动前置条件闭合记录 V1.0》裁决①批准方案）。
--
-- 与 pc 真源逐字段同款（0001:5366-5370
-- cognition_object_tenant_id_project_id_object_key_key 同名列序语义）：
--   * plain UNIQUE = NULLS DISTINCT——匿名草稿（object_key=NULL）多行合法，
--     upsert_object_draft 匿名分支依赖该语义，不得改 NULLS NOT DISTINCT；
--   * 具名草稿获得 DB 级兜底，消除 projc/object.rs 先查后插的并发首插窗口
--     （pc 侧同款约束自 baseline 起即在位）。
--
-- 迁移前检查（2026-11-17，aipm_mig_full / aipm_mig_s1 活库实证）：
--   * (tenant,project,object_key) 重复分组 = 0 / 0；
--   * 现有 projc 对象 2 行（repository_module m1/m2，key 非空互异）——零冲突。
--
-- AGENTS.md 规则 1：无数据迁移、无回填、无兼容分支。

ALTER TABLE projc.cognition_object
    ADD CONSTRAINT cognition_object_tenant_id_project_id_object_key_key
    UNIQUE (tenant_id, project_id, object_key);
