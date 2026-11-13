# Ollama + bge-m3 运行手册（embedding 供给）

> **状态**：已部署（192.168.2.10 用户态安装 + 本机开发端点）——检索架构 V1.0 的 D-1/G-1 落地物。
> **用途**：PC/KB 检索的 embedding 供给（Vector 通道，G-3 门控中）+ 未来相似文档/去重等派生能力。
> **模型**：`bge-m3`（BAAI 多语 embedding，1024 维，中英双语，F16，1.2GB 下载 / 约 1.7GB 常驻内存）。
> **关联**：`pc-kb-unified-retrieval-implementation-taskbook-v1.md` §十一 G-1/G-3 · `docs/reference/test-environments.md`（.10 访问凭据）

---

## 一、当前部署拓扑

| 端点 | 用途 | 说明 |
| --- | --- | --- |
| `http://192.168.2.10:11434/v1` | **生产/联调**（.10 服务器，与 aipm 栈同机） | Ollama v0.40.16，用户态安装，crontab @reboot 自启 |
| `http://127.0.0.1:11434/v1` | **开发/评测**（本机 leo 的 Windows Ollama） | 本机已 pull bge-m3 并验证 1024 维中文 |

两个端点均为 OpenAI 兼容形态：`POST /v1/embeddings`，`{"model":"bge-m3","input":"..."}` → `data[].embedding`（1024 个 float）。

### 1.1 服务器规格与实测

| 项 | 值 |
| --- | --- |
| 内存 | 8GB（2026-11-10 由 4GB 升级——4GB 时 available 仅 449MB 不足以承载） |
| 常驻占用 | bge-m3 加载后约 1.7GB；**空闲自动卸载**（默认 keep-alive 5 分钟，RSS 回落到几十 MB） |
| 延迟（LAN，4 核 CPU 无 GPU） | 冷调用约 2.7s（含模型加载）/ 热调用约 1.2s |
| 磁盘 | 模型 + 二进制共约 2.7GB（`~/.ollama` + `~/.local`）；/tmp 为 3.2G tmpfs **勿放大文件** |

### 1.2 .10 安装形态（用户态，无 sudo）

.10 是最小化安装（无 sudo），故未走 Docker/系统服务：

```
~/.local/bin/ollama            # 二进制（github release ollama-linux-amd64.tar.zst 解压）
~/.ollama/models/              # 模型存储（OLLAMA_MODELS 指向此处）
~/ollama.log                   # 服务日志
crontab -l                     # 含 @reboot 自启行（见 §三.1）
```

---

## 二、验证与接入

### 2.1 健康检查

```bash
# 服务存活 + 版本
curl -s --max-time 5 http://192.168.2.10:11434/api/version

# 模型在位（应见 bge-m3:latest，embedding_length: 1024, capabilities: ["embedding"]）
curl -s --max-time 5 http://192.168.2.10:11434/api/tags
```

### 2.2 embeddings 验证（OpenAI 兼容端点，中文 1024 维）

```bash
curl -s --max-time 60 http://192.168.2.10:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"bge-m3","input":"支付手续费费率为千分之五"}'
# data[0].embedding 长度 = 1024
```

### 2.3 应用接入（Vector 通道启用时，G-3 门控）

- **供给面**：复用 `team_llm` 既有 Provider 管理——注册 provider：`kind=openai_compatible`、`endpoint=http://192.168.2.10:11434/v1`、`model=bge-m3`，消费 purpose=`embedding`（`LlmRetrievalPlanner` 同款模式）。
- **维度**：1024 —— 与迁移草案 `kb.chunk_embedding embedding vector(1024)` 一致，**换模型须同步改列维度并触发全量重嵌**（有效性要素④：embedding_model_version）。
- **降级纪律**：provider 不可用/超时 → Vector 通道自动关闭，BM25 + PageIndex 兜底（检索永不全瘫痪）。
- **门控**：Vector 通道本体属 **G-3**——启用前必须重跑 WP-5 全套评估并证明召回/排序有实际改善；embedding 供给就绪 ≠ Vector 已启用。

---

## 三、运维

### 3.1 自启机制（crontab @reboot）

.10 无 sudo，用用户 crontab：

```
@reboot env OLLAMA_HOST=0.0.0.0:11434 OLLAMA_MODELS=/home/kudi/.ollama/models /home/kudi/.local/bin/ollama serve >> /home/kudi/ollama.log 2>&1
```

手工操作（SSH `kudi@192.168.2.10`）：

```bash
# 启动（前台调试）
OLLAMA_HOST=0.0.0.0:11434 OLLAMA_MODELS=$HOME/.ollama/models ~/.local/bin/ollama serve
# 停止
pkill -f 'ollama serve'
```

> **ssh 后台启动注意**：`nohup ... &` 必须 `< /dev/null` 收尾，否则 ssh 会话挂住不退出（FD 未关闭）。

### 3.2 内存守则（.10 仅 8GB，与 23 容器 aipm 栈同机）

- bge-m3 常驻 ~1.7GB；栈自身 + Ollama 并存时 available 约 2GB——**不要再往上堆常驻服务**。
- 4GB 时代（available 449MB）曾判定不可装机——升级到 8GB 是硬前置。
- OOM 判断：`dmesg | grep -i 'out of memory'`（需 sudo 时用 `docker stats` 旁证）。
- keep-alive 调整：默认 5 分钟空闲卸载；如需常驻（查询频繁场景）设 `OLLAMA_KEEP_ALIVE=-1`，代价是常占 1.7GB。

### 3.3 模型管理（离线搬运配方）

.10 墙 Docker Hub 与 GitHub release；`registry.ollama.ai` 同样不可直连。**离线搬运流程（本机中转）**：

```bash
# ① 本机 pull（本机网络可达；若 ollama pull DNS 失败=进程继承死代理，重启 ollama 服务即愈）
ollama pull bge-m3

# ② 按 manifest 精确打包（只打包 bge-m3 引用的 blobs，不裹其它模型）
#    注意 blob 文件名是 sha256-<digest>（冒号→连字符）
#    产物：manifests/registry.ollama.ai/library/bge-m3/latest + blobs/*

# ③ scp 到 .10 并解包到 ~/.ollama/models/（tar 内相对路径 ./manifests ./blobs）
scp bge-m3-models.tar kudi@192.168.2.10:/tmp/
ssh kudi@192.168.2.10 'tar -xf /tmp/bge-m3-models.tar -C ~/.ollama/models/'
```

> **尺寸必验**：scp 后 `stat -c %s` 与源文件对比（曾有传输截断 30MB 导致 zstd premature end）。**/tmp 是 tmpfs**：大文件放 `~/ollama-install/`（磁盘），放 /tmp 会打满 3.2G 内存盘。

### 3.4 已知坑位（部署实录）

1. **/tmp = 3.2G tmpfs**：安装包/解压放 /tmp 直接 No space left → 移家目录操作。
2. **.10 无 sudo**：系统级安装/Docker 全不可用 → 用户态 tar 解压 + crontab @reboot。
3. **Docker Hub 走欠费加速器 402**、.10 墙 GitHub release（307→github 不通）→ 大文件一律本机下载 scp。
4. **本机 ollama pull 报 DNS 失败但 curl 通**：ollama 服务进程继承死代理 → 重启 ollama 服务即愈。
5. **release 资产名**：`ollama-linux-amd64.tar.zst`（新版 zstd 压缩，非 .tgz/.tar.gz）；.10 有 /usr/bin/zstd 可解。
6. **scp 截断**：超时转后台后重复传输可能竞争 → 传输完必验字节数。

---

## 四、版本与升级

- 当前：Ollama **v0.40.16** / bge-m3 **latest**（F16，566.70M 参数，ctx 8192，dim 1024）。
- 升级流程 = §3.3 同款（本机下载新版 release → scp → 解压覆盖 `~/.local/bin` → pkill 后重启）。
- **换 embedding 模型 = 破坏性变更**：维度或语义空间变化 → `kb.chunk_embedding` 全量重嵌 + `embedding_model_version` 升版（有效性要素④）→ 需重跑 WP-5 评估。
