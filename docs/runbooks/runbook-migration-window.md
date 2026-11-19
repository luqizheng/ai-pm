# Runbook：数据库迁移执行窗口约束（DDL × close 分类事务）

> 状态：**生效约束**（2026-11-19，GAP-32 验收关闭登记；契约源=《Q10-R1-FIX 实施与验证报告 V1.0》§五.3 + 审批附件补充 B.3 +《GAP-32 验收裁决 V1.0》条目 4）。
> 对象：所有对 `pc.*` / `projc.*` / `kb.*` / `core.*` 等业务 schema 的迁移（ALTER/DROP/建索引 CONCURRENTLY 除外规则见下）。

---

## 一、为什么有这条约束

Q10-R1-FIX（B''/II-A）起，`close_report` 的 Err 分支会开启**分类事务**：
`BEGIN → pg_advisory_xact_lock('pc_publish:'||project_id) → 单条 JOIN（读 pc/projc 的
proposal + write_operation 四表）→ COMMIT`（毫秒级）。

普通 SELECT/JOIN 持**表级 ACCESS SHARE** 锁——与一切 DML 并发兼容，但与任何取得
**ACCESS EXCLUSIVE** 的 DDL（`ALTER TABLE` / `DROP TABLE` / 非 `CONCURRENTLY` 的
`CREATE INDEX` / 加 NOT NULL 约束等）**互斥**：

- DDL 先到：分类事务的 JOIN 被阻塞至 DDL 提交（ advisory 已持、JOIN 排队）；
- 分类先到：**DDL 排队**，并进一步阻塞其后**所有**该表上的 DML（ACCESS EXCLUSIVE
  排队会挡住后续 ACCESS SHARE）——在线迁移会造成请求堆积。

运行面无 DDL 是常态；本约束只针对迁移执行时刻。

## 二、约束（必须遵守）

1. **迁移仅在应用实例停止或维护窗口执行**（现有部署惯例即如此）；禁止在业务流量
   在线时对上述 schema 跑 `ALTER/DROP` 类迁移。
2. 迁移前置检查：确认无活跃 close（`integration.repo_analysis_report` 无 `analyzing`
   / `analyzed` 在途报告亦可作辅助信号；硬检查见 §三）。
3. `CREATE INDEX CONCURRENTLY` 不取 ACCESS EXCLUSIVE，可在维护窗口外使用（其余
   DDL 不适用此豁免）。

## 三、迁移前 60 秒检查（docker exec 进入 aipm 库）

```bash
# 1) 是否有未决的 ACCESS EXCLUSIVE 等待或长事务（空输出=安全）
docker exec aipm-dev-postgres psql -U aipm -d aipm -c "
SELECT pid, state, wait_event_type, xact_start, query
  FROM pg_stat_activity
 WHERE datname = current_database()
   AND (wait_event_type = 'Lock' OR xact_start < now() - interval '1 minute');"

# 2) 在途分析报告（analyzing/analyzed = close 门在占，宜等关闭或窗口期）
docker exec aipm-dev-postgres psql -U aipm -d aipm -c "
SELECT id, status, updated_at FROM integration.repo_analysis_report
 WHERE status IN ('analyzing','analyzed');"
```

## 四、违规后果与恢复

- 症状：迁移卡住（等 ACCESS SHARE 释放）或 close 请求堆积（等 ACCESS EXCLUSIVE）。
- 处置：`SELECT pg_terminate_backend(<阻塞方 pid>)` 前先辨明身份——**宁可回滚迁移
  （迁移文件每文件独立事务，重放幂等面见各迁移头注）也不要杀业务 close**；
  `close_report` 的分类事务毫秒级自愈，通常等数秒即可。
