# 客户主数据 v1（0017）

> 状态：🟡 **已实施并验证，但后续工作延后（非必须完成项）**
>
> 已落地部分保持现状、不回滚：迁移 0017 / pm-api `/v1/pm/customers` / 前端客户列与建项选择器。
> 延后部分（见 §6 遗留差距 G1–G4）为**非阻塞、非必做**，不排入当前主路径；何时重启由用户决定。
>
> 一句话：客户从「建项目的副作用」升级为上层实体，一个客户挂 N 个项目。

## 1. 问题

改造前 `0001_core.sql` 已有 `core.customer` 与 `core.project.customer_id`，但
`bootstrap_project` 以 `name = project.code` 隐式建客户，实际语义是
**1 项目 = 1 客户**。客户表只有 `name / status / created_at` 三列，没有 code、
没有维护接口、前端只有裸 `customer_id`，UI 与文档都不出现「客户」。

反例：为「中国某某银行」做了 5 个项目，旧模型会得到 5 个互不相关的客户，
无法按客户聚合、无法改名、无法在客户维度承载跨项目的上下文。

## 2. 裁定（经确认）

| # | 决策 | 结论 |
|---|---|---|
| 1 | 建模层级 | 客户是**一等实体**（独立聚合 + 外键），不是 project 上的文本标签 |
| 2 | 唯一标识 | 新增 `code` 部分唯一索引作为业务幂等键；`name` 不唯一（同名不同主体是常态） |
| 3 | 父子层级 | v1 单层，预留 `parent_id` 可空自引用，不做集团树 |
| 4 | 项目换客户 | 允许，但属命令通道动作，须审计；`revision` 递增 |
| 5 | 客户视图可见性 | 只列「我参与的项目」 |

## 3. 模型

```sql
core.customer
  id           uuid pk
  code         text            -- 业务唯一键，部分唯一索引 WHERE code IS NOT NULL
  name         text not null   -- 客户全称（银行类常带「股份有限公司XX分行」）
  short_name   text            -- 列表与下拉优先展示
  industry     text
  notes        text
  parent_id    uuid null -> core.customer(id)
  status       text            -- active / archived / placeholder
  created_at   timestamptz
  updated_at   timestamptz
```

状态语义：

- `active`：正常，可挂新项目。
- `archived`：归档。禁止新建项目（`bootstrap_project_for_customer` 会拒绝），
  历史项目仍可读。**不级联删除项目**——删除客户会让历史项目失去归属。
- `placeholder`：0017 之前由项目 code 隐式生成、尚未归类的客户。UI 一律渲染为
  「未归类 · <name>」，不得当作真实客户名展示。

## 4. 接口契约

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/v1/pm/customers?q=&status=&page=&size=` | 列表，`q` 匹配 name/code/short_name；返回 `project_count` |
| POST | `/v1/pm/customers` | 建客户：`code`、`name` 必填；`short_name`/`industry`/`notes`/`parent_id` 可选；code 冲突 409 |
| GET | `/v1/pm/customers/{id}` | 详情（含 `project_count`） |
| PATCH | `/v1/pm/customers/{id}` | 字段缺省即不改；可空文本传空串即清空；`status` 仅接受 active/archived |

派生变更：

- `GET /v1/pm/projects` 新增 `customer_id` 过滤，返回体新增 `customer_name`、
  `customer_status`（JOIN `core.customer`）。
- `POST /v1/pm/projects` 新增可选 `customer_id`：传了走
  `bootstrap_project_for_customer`（校验客户存在且未归档，失败 400）；不传保留旧
  兼容路径，客户落为 `placeholder`。

前端入口：

| 位置 | 作用 |
|---|---|
| `/customers`（静态路由，菜单外） | 客户列表 + 新建/编辑/归档；`placeholder` 客户在此补 code 与全称 |
| 项目列表 | 新增「客户」列、客户筛选下拉、跳转客户管理的入口 |
| 导入向导第一步 | 「所属客户」下拉，仅列 `active` 客户；留空落 `placeholder` |

## 5. 存量迁移

迁移 0017 只做两件事：补列、把「客户名 == 该项目 code」的历史客户标为
`placeholder`。

**不做自动归并**：系统无从判断 `AIDEMO` 属于哪家客户，任何启发式都在制造错误数据。
归类动作留给人在 UI 里完成（PATCH 改名 + 补 code，或把项目 PATCH 到目标客户上）。

## 6. 遗留差距（已知，非缺陷 · 🟡 整体延后，非必做）

以下四项**不阻塞任何现有功能**，也**不排入当前主路径**。需要时再重启，重启前先回到 §2
重新确认裁定是否仍成立。

| # | 差距 | 影响 | 处置 |
|---|---|---|---|
| G1 | 建项时客户**可留空**，留空落 `placeholder` | 与裁定「建项须选客户」不完全一致 | 🟡 延后；重启时做行内「新建客户」后收紧为必选 |
| G2 | 客户详情页未做——「该客户全部项目」通过项目列表的客户筛选替代 | 少一次跳转，功能等价 | 🟡 延后；与 G3 一起做 |
| G3 | 按 `project_member` 过滤的可见性未实现——当前 auth 层只验签，未把 principal 映射到 `core.principal` | 裁定 5 暂无法落地，客户维度仍是全局可见 | 🟡 延后；属独立切片（principal 映射） |
| G4 | 客户级 AI 上下文（术语表、验收偏好、历史红线） | 无 | 🟡 延后至 v2；与「项目隔离」原则的张力需单独裁定 |

> G1 是**已知的行为偏差而非缺陷**：留空时 UI 显示为「未归类 · <项目 code>」，不会被误读成真实客户。
> 项目 tag 归纳轴是另一条正交轴，2026-11-01 已裁定放弃，不在本文档范围内、也不作为遗留项待办。

## 7. 验证

```bash
cargo check --workspace --all-targets
cd frontend && npx vue-tsc --noEmit --skipLibCheck
```

数据库侧：`sqlx::migrate!` 在 `apps/pm-api/src/main.rs` 启动时执行 0017；
`scripts/seed-demo.sql` 的演示客户已补 `code = 'DUNPAI'`。
