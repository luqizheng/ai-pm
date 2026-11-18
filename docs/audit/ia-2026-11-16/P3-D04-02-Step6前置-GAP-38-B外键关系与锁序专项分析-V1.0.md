# P3-D04-02 Step 6 前置 · GAP-38-B 外键关系与锁序专项分析 V1.0

- 编制日期：2026-11-18
- 性质：**独立分析与裁决输入（零实施）**——负责人裁决口径：「先分析，暂不实施；分析结果形成独立裁决输入，再决定外键实施范围」
- 基准：GAP-38-A 落地后（0036 PK×18+业务身份 UNIQUE×4）；pc 镜像 FK 集=候选集基准
- 结论效力：本报告只输出关系清单/锁序影响/行为口径/范围建议——**不批准任何 FK 实施**；实施须负责人基于本报告另行裁决

---

## 一、候选集基准（pc 镜像）

GAP-38-A 后两域表结构已对称（18 表）。projc 候选 FK 集=pc 同名表 FK 集（pc 镜像域 FK 全集恰为这 18 表上的 79 条边）：

| 表 | pc FK 数 | 表 | pc FK 数 |
|---|---|---|---|
| approval | 4 | proposal | 4 |
| cognition_object | 6 | proposal_member | 7 |
| cognition_revision | 4 | relationship | 8 |
| cognition_version | 4 | relationship_revision | 11 |
| conflict | 4 | source | 2 |
| conflict_object | 4 | version_member | 7 |
| document | 2 | write_operation | 3 |
| document_revision | 4 | object_type / relationship_type | 0 |

**引用簇归类**：①core 引用（tenant/project/principal——多租户与主体门背书，pc 全表具备）②身份/内容簇（object↔revision↔document↔document_revision）③提案簇（proposal↔proposal_member↔source↔approval）④版本簇（version↔version_member↔object/relationship/revision/relationship_revision）⑤冲突簇（conflict↔conflict_object↔object/relationship）⑥字典引用（object_type/relationship_type，pc 侧经唯一列引用）。

## 二、ON DELETE / ON UPDATE 行为口径

pc 真源 79 条边**全部未声明 ON DELETE/ON UPDATE**（取样核验：approval_decided_by_fkey 等均为默认）→ 即 **NO ACTION**（限制式；无级联删除/置空）。

**建议口径（若实施）**：projc 逐字镜像 pc 默认（NO ACTION）——理由：①与「append-only/终态不可变」领域语义一致（引用行不可静默消失）；②pc 侧同语义已运行无回归；③避免 ON DELETE CASCADE 引入跨域级联删除（与「共享技术基础设施不得产生共享业务权威」红线相悖）。

## 三、锁序与级联锁影响（对照 Step3 锁分析前提）

Step3 并发分析对 projc 写路径的前提=「projc 无 FK=无 FK 级联锁」（E-31 族）。补 FK 后该前提改变，逐写路径影响：

| 写路径 | 现行锁序（§17.3） | FK 插入检查新增锁 | 影响 |
|---|---|---|---|
| decide（两域） | proposal FOR UPDATE→approval FOR UPDATE | 对 core.principal（decided_by）行 SHARE 锁 | 单行新增；与既有键序一致，低风险 |
| publish（两域） | advisory→proposal FOR UPDATE→approval FOR UPDATE→…→version INSERT→members INSERT→账本 | version/members 各自引用行（object/relationship/revision/relationship_revision/proposal）SHARE 锁；这些行在 publish 事务内已被 FOR UPDATE 或本事务插入 | 锁对象与既有锁序同向（先强锁后 share 校验），无反向边 |
| draft upsert（两域） | object FOR UPDATE→revision/document INSERT | core.tenant/project/principal SHARE 锁+document 存量行（同事务插入） | 低风险 |
| propose/identity 起草段 | 候选查询（只读）→账本→proposal/members INSERT | source/object/relationship SHARE 锁（候选已只读过） | 低风险 |
| close 批量 | 报告事务+逐提案 publish（同池不同连接） | 同 publish | 与 close 既有锁序同向 |

**风险点**：①FK 检查锁与「先查后插」服务层模式叠加可能放大锁竞争窗口（实测验证义务）；②批量 members INSERT 的引用行 SHARE 锁需与 publish 既有 FOR UPDATE 键序保持一致（当前同向）；③未来任何 DELETE 路径（当前零）将受 NO ACTION 限制（与 GAP-37 系守卫同向，非冲突）。

**结论**：逐路径分析未见锁序反转；但「分析未见」≠「无影响」——实施必须以 GAP-30/30 同款并发用例（并发 publish+upsert 交错）实测背书。

## 四、实施范围建议（若负责人裁决实施）

1. 单独迁移（拟 0037），按簇分批落列：core 引用簇→身份/内容簇→提案簇→版本簇→冲突簇；
2. 实施前逐边查重/孤儿预检（引用行缺失即停报告）；
3. 并发用例背书（交错 publish/upsert/decide 压测一轮）；
4. 回滚=逐簇 DROP CONSTRAINT（反向依赖序）。

## 五、零实施声明

本报告为分析交付：未修改代码/Schema/Migration/API/测试/数据库；FK 未实施、未预占迁移编号；实施与否、范围与批次由负责人基于本报告另行裁决。
