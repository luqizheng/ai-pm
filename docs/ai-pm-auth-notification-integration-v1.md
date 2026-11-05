# ai-pm 基础功能与后端对接（认证 / 资料 / 通知）v1

> 日期：2026-11-05
> 范围：`ai-frontend`（前端） + `apps/pm-api`（通知桥接，如需）
> 上游依赖：`low-memory-micro-services` iam-service（认证/资料）、notification-service（公告）
> 设计依据：`low-memory-micro-services/docs/superpowers/specs/2026-11-05-iam-self-service-auth-design.md`

## 一、前因后果

### 1.1 现状

| 能力 | 前端 | 后端 |
|---|---|---|
| 登录 | `LoginView` 已按 Figma 落地 | iam 有 `POST /iam/auth/login`，前端未接 |
| 注册 | 无注册页，保留邀请页 | iam `register` 仅允许零用户，无邀请端点 |
| 忘记密码 | 页面已落地 | iam 无端点、无邮件能力 |
| 重置密码 | 页面已落地（`?token=`） | 只有孤儿 DTO |
| 修改密码 | 安全页假保存 | iam `POST /iam/auth/password` 已就绪 |
| 用户资料 | 资料页假保存、硬编码 | iam 有 profile 列，无 `/users/me` |
| 通知 | 铃铛/下拉/中心已落地 | 全部是 `demo.ts` 演示数据；lmms 公告链路完整但未接 |

### 1.2 本轮目标

1. 登录、忘记密码、重置密码、接受邀请全部走真实 iam 接口。
2. 个人资料（昵称 + 头像）与修改密码真实保存。
3. 通知以 lmms 公告为主：铃铛、未读数、通知中心、单条已读、全部已读全部接真实数据；
   新增管理员公告管理页（创建 / 发布 / 撤销）。
4. 未登录访问业务页重定向登录。

### 1.3 非目标

- 公开自助注册。
- 邮箱/手机号变更与验证。
- PM 项目事件自动桥接成公告（下一轮）。
- 通知 WS 实时推送（先轮询/进入页面拉取）。

## 二、前端落点

| 模块 | 文件 | 改动 |
|---|---|---|
| 认证网关 | `src/api/auth.ts` | `mockAuthGateway` → `iamAuthGateway` |
| iam 适配 | `src/api/base/iam.ts` | 已存在，补调用 |
| 登录页 | `src/views/auth/LoginView.vue` | 保持 UI；错误码映射到现有文案 |
| 忘记/重置/邀请 | `src/views/auth/*.vue` | 只改 store 调用，UI 不动 |
| 认证 store | `src/stores/auth.ts` | 会话改为 `user` + token 持久化；补 logout |
| 路由守卫 | `src/router/index.ts` 或新增 `guard.ts` | `beforeEach`：无 token 且非 auth 路由 → `/auth/login` |
| 个人资料 | `components/settings/sections/PersonalProfileSection.vue` | 接 `GET/PUT /iam/users/me`；头像上传 |
| 个人安全 | `components/settings/sections/PersonalSecuritySection.vue` | 接 `changePassword` + `sessions` |
| 通知 store | `src/stores/notifications.ts` | loader 改为真实 `notification.listMine()` |
| 通知映射 | `src/api/notifications.ts`（新增） | `Announcement → NotificationRecord` |
| 公告管理 | `src/views/admin/AdminAnnouncementsView.vue`（新增） | 列表 / 创建 / 发布 / 撤销 |
| 导航 | `src/router/navigation.ts` | 管理分组加「公告管理」 |

## 三、数据契约

### 3.1 认证

```ts
// 登录
POST /v1/iam/auth/login  { username, password } → LoginResponse
// 忘记密码
POST /v1/iam/auth/forgot-password { email } → 204
// 重置密码
POST /v1/iam/auth/reset-password { token, new_password } → 204
// 接受邀请
POST /v1/iam/auth/invitations/accept { token, password, nickname? } → LoginResponse
// 自助资料
GET  /v1/iam/users/me → UserResponse
PUT  /v1/iam/users/me { nickname?, avatar? } → UserResponse
```

### 3.2 通知

```ts
GET  /v1/notification/announcements?active=true → Announcement[]
GET  /v1/notification/unread-count → { count }
POST /v1/notification/announcements/{id}/read → { ok, changed }
POST /v1/notification/announcements/read-all → { ok, changed }   // notification-service 新增
POST /v1/notification/announcements → { id }                     // admin
POST /v1/notification/announcements/{id}/publish → { id, status, user_count }  // admin
POST /v1/notification/announcements/{id}/revoke  → { id, status }              // admin
```

`Announcement` 需补 `read: boolean`（服务端按 `ntf_announcement_reads` 左连接）。

### 3.3 前端记录映射

| Announcement | NotificationRecord |
|---|---|
| `id: number` | `String(id)` |
| `title` | `title` |
| `body` | `description` |
| `publish_at` | `timestamp` |
| `pinned` | `requiresAttention` |
| `read` | `read` |
| 固定 `systemAlerts` | `category`（公告不区分 PM 六类） |
| 无 | `object/openLabel/target` 置空字符串 |

## 四、实施顺序

| 阶段 | 内容 | 验收 |
|---|---|---|
| F1 | auth 网关真实化 + 路由守卫 + logout | 真实账号可登录；未登录深链跳登录；退出后 token 清除 |
| F2 | 忘记/重置/邀请页接真实接口 | MailHog 收信 → 点链接 → 改密/激活成功 |
| F3 | 资料页 + 安全页 | 昵称保存后刷新仍在；改密后旧密码失效；会话列表可见/可吊销 |
| F4 | 通知接真实公告 + 已读/全部已读 | 未读数与公告状态一致；已读刷新后仍是已读 |
| F5 | 管理员公告管理 | 创建→发布→用户可见；撤销→用户列表消失 |
| F6 | i18n / 单测 / 门禁 | `pnpm test:unit`、`typecheck`、`check:i18n`、`check:prism-tokens` 全 exit 0 |

## 五、风险

| 风险 | 缓解 |
|---|---|
| iam 返回的 `UserInfo` 不含组织名，登录后顶栏组织芯片无法回显 | 从会话态用租户 id/默认名占位；不伪造数据 |
| 通知已读状态需后端补 `read` 字段 | 先改 notification-service 再改前端，避免前端临时假状态 |
| 头像上传依赖 file-service | 先复用 `src/api/base/file.ts`；上传失败保留原头像并提示 |
| 路由守卫误伤 `/auth/*` 与公开设计页 | 白名单以 `route.meta.layout === "auth"` + 显式路径表为准 |
| 现有 `mockAuthGateway` 被测试引用 | 测试改为注入 fake gateway，不依赖 mock 实现 |

## 六、验收清单

- [ ] 登录成功写入四键 token，刷新页面仍登录。
- [ ] 退出登录调用 `/iam/auth/logout` 并清空本地会话。
- [ ] 忘记密码恒显示「已发送」，MailHog 有信；重置链路可用。
- [ ] 邀请链接可设置密码并直接进入工作区。
- [ ] 资料页昵称/头像保存后重进仍生效；邮箱手机号只读。
- [ ] 修改密码成功后旧口令登录失败。
- [ ] 通知中心显示真实公告；单条已读/全部已读刷新后保持。
- [ ] 管理员可创建/发布/撤销公告，普通用户看不到管理入口。
- [ ] 前端四项门禁 exit 0。