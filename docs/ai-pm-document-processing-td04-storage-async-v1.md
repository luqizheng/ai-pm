# Document Processing · TD-04 对象存储与异步任务设计 — V1

**上游：** TD-01（job/artifact 表 + lease 列）、TD-02 §3（API）、规格 §5/§7.3/§7.4/§10.2
**两条硬结论：** ①存储**复用 file-service**（RustFS 托管，docproc 不直连 S3）；②异步队列**不做新队列**——`docproc.processing_job` 表即队列（规格 §7.4「不建议为文档转换单独引入复杂分布式工作流平台」；复用现有任务基础设施 = 沿用 outbox 同款的 SKIP LOCKED + lease 数据库原语）。

---

## 1. 存储生命周期

| 对象 | 写入者 | 保留 | 删除路径 |
|---|---|---|---|
| 原始文件（每 version 一份） | pm-api 上传时 | **永久**（规格 §15.4：不得因转换失败丢失） | V1 无用户删除入口；U-05 保留策略裁决后由生命周期任务执行 |
| 转换产物 artifact | pm-worker | **永久**（D-06：历史产物保留，转换器升级不覆盖） | 同上（U-09） |
| 图片/附件 | pm-worker | 跟随所属 artifact 生命周期 | 同上 |
| 导出/比较产物 | pm-worker | 跟随生成它的 job | 同上 |

- V1 **不做**对象存储层生命周期规则（过渡期零删除、零 GC）；U-05/U-09 闭合时一次性引入，避免半吊子清理。数据库层已由 append-only 触发器兜底，对象层"孤儿文件"（行被手工删的极端场景）V1 接受，由对账脚本（P1）兜底。
- file-service 归属：所有 docproc 上传 metadata 打 `domain=docproc` 标记（file-service 侧可按 tenant 过滤列表），文件名约定 `{kind}:{tenant}:{document}:{version|artifact}.{ext}`。

## 2. 上传/下载路径

- **上传**（pm-api）：multipart 接收（≤100MiB，`DefaultBodyLimit` 对齐 files.rs）→ sha2 校验和 → `FileClient.upload_file`（per-tenant 服务 JWT，每请求独立 client，防租户串号——files.rs 已实证该模式）→ `document_version` 行。
- **下载**：`FileClient.create_presign`（SigV4 预签 URL，浏览器免 JWT 直取）→ docproc API 只发**短时效 URL**，不代理字节流（大文件不占 pm-api 内存）。

## 3. Job 生命周期（状态机）

```text
pending ──claim──► processing ──► completed | partial_success
   ▲                  │    │
   │  claim 过期回收   │    └──► failed ──retry(回 pending, attempts+1)──┐
   └──────────────────┘                                                 │
                                                                        └─► attempts ≥ max(3) 停止自动重试
pending/processing ──cancel API──► cancelled（终态，唯一索引放行新任务）
```

- **状态迁移约束**（应用层 + 注释说明，不加触发器）：终态（completed/partial_success/failed/cancelled）除 failed→pending 重试外不可回退；processing 只能从 pending 进入。
- 取消：仅 pending/processing 可取消；processing 取消 = 置 cancelled，worker 提交时发现终态则丢弃产物写入（先查后写 + 行级 row_revision 乐观锁双保险）。

## 4. Worker 领取与执行（幂等/重试核心）

```sql
UPDATE docproc.processing_job SET status='processing', lease_owner=$worker,
       locked_until=now()+interval '5 minutes', started_at=now(), attempts=attempts+1
WHERE id = (SELECT id FROM docproc.processing_job
            WHERE status='pending' OR (status='processing' AND locked_until < now())
            ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED)
RETURNING *;
```

- **租约**：5 分钟（规格 §10.2 大文档上限对齐）；执行超时 = 租约过期被其他 worker 重领（at-least-once），**处理器必须幂等**：产物写入以 `(job_id, artifact_type, checksum)` 唯一约束吸收重复（ON CONFLICT DO NOTHING 幂等落库）。
- **执行内超时**：单 job 解析 wall-clock 上限 120s（tokio::time 包裹），超时 → failed `parser_timeout`（不等租约被动过期，主动快速失败）。
- **执行序**：claim → 拉原始字节（file-service 下载，≤100MiB 内存上限）→ adapter.parse → 图片/产物逐一 upload file-service → 单事务写 artifact + source_mapping + job 终态 + outbox 事件（事务原子——事务回滚则事件与产物状态一致回退，对象存储侧残留由唯一约束在重试时吸收）。
- **重试退避**：failed 不自动即时重试；`attempts < 3` 时由 worker 将行回 pending（立即）——简单优先，退避由租约+SKIP LOCKED 天然形成；`attempts ≥ 3` 停留在 failed，等待人工/API 触发 `POST /jobs/{id}/retry`（重置 pending + attempts 清零，仅 failed 可调）。
- **产出防重**：入队幂等由 TD-01 `uq_job_active_config` 兜底（重复 POST 返回既有 job）；执行幂等由 artifact 唯一约束兜底；事件幂等由 outbox idempotency_key 兜底。三层各自独立，任一层重复都不产生业务副作用。

## 5. 资源限制（安全承接，规格 §9.6/9.7/9.8）

| 限制 | 值 | 实施点 |
|---|---|---|
| zip 单条目解压比 | ≤100x | docproc_parse 容器打开层（逐 entry 检查，超限即拒） |
| zip 总解压量 | ≤512MiB | 同上累计 |
| 原始文件大小 | ≤100MiB | API 层（3.1） |
| 单 job 解析时长 | ≤120s | worker 执行包裹 |
| 解析内存 | 峰值 ≈ 原始×解压比上限（512MiB 封顶） | 守卫间接保证 |
| 外部网络 | **零**（纯 Rust 内存解析，无任何外呼） | 结构性保证 |

## 6. 配置面（U-04 闭合）

环境变量（均有默认值，零配置可跑）：`DOCPROC_MAX_UPLOAD_BYTES`（默认 100MiB）、`DOCPROC_JOB_LEASE_SECS`（300）、`DOCPROC_JOB_TIMEOUT_SECS`（120）、`DOCPROC_MAX_ATTEMPTS`（3）、`DOCPROC_WORKER_POLL_INTERVAL_MS`（1000）。Team 配额（U-04 后半）V1 不做，留 U-04。

## 7. 可观测性（规格 §10.3）

- job 行自含：parser_name/version、attempts、started/completed、error_code/detail——处理耗时/成功率/失败原因直接可查。
- tracing：worker 每 job 一条结构化日志（job_id、文档类型、耗时、产物计数）；**只记元数据，不记文档正文**（规格红线）。
- 进度：progress 0→100 在「解析完成 60 / 产物上传 90 / 收尾 100」粗粒度推进（V1 不做细粒度）。
