# P3-D04-02 Step 6 前置 · GAP-37-C 与 N-1 设立裁决输入 V1.0

- 编制日期：2026-11-18
- 性质：**裁决输入材料（零实施）**——供负责人就两项设立事宜拍板；推荐方案≠裁决
- 依据：总账 V1.6 §十.2（GAP-37-C 核对=无既有 GAP 对应）/§十.3（N-1 核对=无既有 GAP 对应，建议编号 GAP-38）+《Step5-实施与验收关闭记录 V1.0》§七
- 术语一致性：GAP-37-C=拟设于 GAP-37 槽内第三子项；GAP-38=拟新占空闲槽位

---

## 一、GAP-37-C：projc.cognition_version DELETE 守卫缺位

### 1.1 事实与证据（SCHEMA/CODE FACT，2026-11-18 实测）

| 项 | 证据 |
|---|---|
| pc 真源 | `trg_pc_version_delete BEFORE DELETE ON pc.cognition_version FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update()`（0001:7050）——已发布版本行 DELETE 守卫 |
| projc 缺位 | projc.cognition_version 仅 `trg_projc_cognition_version_immutable BEFORE UPDATE`（0025:435，UPDATE 单面）；**无 DELETE 守卫** |
| 现状写路径 | projc publish 纯 INSERT（projc/publish.rs:486/505/519）；`DELETE FROM projc.cognition_version` 全仓 0 命中 |
| 同族先例 | GAP-37-A/B（Batch 1 已验收）：同族镜像保护面，验收模式=探针+清单+回归 |
| 完备性影响 | T-2 镜像清单 D 组现行注记行（pc 有/projc 无，待裁决）——设立后升级为 A 组行 |

### 1.2 设立范围草案（若批准）

- Migration 0035：`CREATE TRIGGER trg_projc_version_delete BEFORE DELETE ON projc.cognition_version FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();`（pc 真源逐字同款；函数 0025:329 在位，零新函数；纯增量零数据迁移）；
- T-2 镜像清单更新：D 组「trg_pc_version_delete」行升 A 组（两域一致）；projc 12→13；
- 测试面：T-1 扩 DELETE 探针（projc.cognition_version）；GAP-30/回归不受影响；
- 回滚=DROP TRIGGER。

### 1.3 选项与影响

| 选项 | 内容 | 影响 |
|---|---|---|
| **A=设立 GAP-37-C（推荐）** | 上表范围；可与下一批次同行（纯增量小时级） | T-2 清单/测试一处更新；镜像保护面完全对称；未来开放任何 DELETE 路径前防御已在位 |
| B=不设立 | 维持 D 组注记（pc 有/projc 无，长期登记） | 镜像面保持已知不对称；未来 DELETE 路径开放前须补（届时强制前置） |
| C=并入未来批 | 记入待办不建子项 | 同 B 的窗口风险 |

**推荐=A**：成本最低、与 GAP-37-A/B 同构同验收模式、消除镜像清单最后一条同族不对称（trg_pc_version_delete 是 D 组唯一非 relationship 的例外行）。

### 1.4 验收标准草案（若设立）

①UPDATE 探针维持拒绝（既有）+DELETE 探针拒绝（新增实测）；②T-2 清单升级后全量断言绿；③projc_formal_chain 全量回归；④泄漏检查。与 GAP-37-A/B 验收三类+追加维度同口径。

---

## 二、N-1：projc 全域 0 主键/0 外键（拟议编号 GAP-38）

### 2.1 事实与证据（2026-11-17/18 实测）

| 项 | pc | projc |
|---|---|---|
| 主键 | 17 表全 PK（0001 ALTER TABLE 段） | **0** |
| 外键 | 79 FK | **0**（Step3 文档「外键=0」对 pc 系取证失误——勘误已登记总账 §十.4） |
| 唯一约束/索引 | pkey+UNIQUE 族 | 10 个部分唯一索引+0026/0028 两约束+0029（GAP-04）+Batch4a/4b 增列 |
| **唯一索引覆盖** | 全表覆盖 | **9/17 表无任何唯一索引**（identity/版本/账本类核心表在列） |
| 登记状态 | —— | 总账 V1.4~V1.6 §十.3 核对=无既有 GAP 对应；0025 头注未声明省略意图 |

### 2.2 风险与影响面

1. **身份无 DB 兜底**：`WHERE id = $1` 全部顺序扫描（9 表无唯一索引）；id 理论可重复（gen_random_uuid 冲突概率趋零，但无约束背书）。
2. **性能面**：dev 规模零感知；数据增长后 seq scan 放大。
3. **并发分析前提**：Step3 锁分析引用「projc 无 FK=无 FK 级联锁」（E-31 族）——**若补 FK，该分析前提对 projc 写路径改变**（PC 侧本就有 79 FK，分析在 PC 面成立不受影响）→FK 补齐须先重审 projc 写路径锁序（非冻结裁决重开，但属分析证据更新）。
4. **写路径面**：现状纯 INSERT+decide 单 UPDATE；GAP-01/27 落地后起草段账本已同事务化——无直接覆盖风险（同 GAP-37 立项时论证口径）。

### 2.3 选项与影响

| 选项 | 内容 | 影响 |
|---|---|---|
| **A=设立 GAP-38 并拆两步（推荐）** | **GAP-38-A**：17 表补 PRIMARY KEY(id)+核心表补 UNIQUE（先查重后建约束；纯增量，无并发前提变更）→**GAP-38-B**：FK 补齐（须先出 projc 锁序/级联影响专项分析并过负责人确认后实施） | A 步小时级低风险；B 步独立可裁；把「分析前提冲突」显式化而非静默 |
| B=整体不设立 | 登记为已知架构差异（总账 §十.3 维持） | 唯一索引缺失面持续存在；未来写路径扩展（Batch 后续）在无兜底表上叠加 |
| C=仅登记不建 GAP | 同 B | 无编号承载，验收无锚 |

**推荐=A（拆两步）**：38-A 与 GAP-37-C 同批（同为纯增量小时级）；38-B 独立裁决（锁序专项先行）。

### 2.4 验收标准草案（若设立）

- 38-A：①9 表+全 17 表 PK 落列（pg_constraint 实测）；②唯一索引覆盖复测=0 裸表；③补前查重=0（非 0 即停报告）；④全量回归绿；
- 38-B：①FK 清单逐表批准后落列；②projc 写路径锁序专项分析先行并过确认；③回归+探针。

---

## 三、裁决清单（负责人拍板点）

| # | 事项 | 选项 | 推荐 |
|---|---|---|---|
| 1 | GAP-37-C 设立与否 | A 设立（槽内子项 C）/B 不设立/C 并入未来批 | A |
| 2 | 实施时机（若设 C） | 随下一小批/与 38-A 同批 | 与 38-A 同批 |
| 3 | GAP-38 设立与否 | A 设立并拆 38-A(PK+UNIQUE)/38-B(FK)/B 不设立/C 仅登记 | A 拆两步 |
| 4 | 38-B 的锁序专项分析 | 先分析后实施/暂缓仅 38-A | 先分析后实施 |

## 四、零实施声明

本材料为文档交付：未修改代码/Schema/Migration/API/测试/数据库；两条 GAP 均未设立、未预占编号（GAP-37-C 以子项形态、GAP-38 未占槽）；实施在设立裁决+授权后另行进行。
