//! 仓储 port（真仓储层骨架，任务书批 B / 裁决 D1）。
//!
//! 分层契约：**trait + 行/读模型在本 crate**（复用 `crate::rows` 与既有
//! Serialize 惯例），**Postgres 实现在 pm-infrastructure**（沿用 docproc_store
//! 先例：R7 端口在 application、适配器在 infrastructure；依赖方向
//! infrastructure → application 单向）。
//!
//! 错误首版收敛为 `sqlx::Error`（pm-api 的 `ApiError` 已有 `From` 映射）；
//! 需要 NotFound/Conflict 语义的域在迁移时逐步引入 `RepoError`，不在骨架期
//! 预设抽象。

pub mod acceptance_read;
pub mod ai_team_monitor;
pub mod baseline;
pub mod cognition;
pub mod identity;
pub mod customer;
pub mod design_guard;
pub mod guard_query;
pub mod platform_ops;
pub mod kb_pc_query;
pub mod runs;
pub mod team_members;
pub mod lists;

pub mod viz_read;

pub use acceptance_read::AcceptanceReadRepo;
pub use ai_team_monitor::AiTeamMonitorRepo;
pub use baseline::{BaselineRepo, BaselineRepoError};
pub use cognition::{CognitionRepo, CognitionRepoError};
pub use customer::CustomerRepo;
pub use design_guard::DesignGuardRepo;
pub use guard_query::GuardQueryRepo;
pub use identity::{IdentityRepo, IdentityRepoError};
pub use kb_pc_query::KbPcQueryRepo;
pub use lists::{ListPage, PmListRepo};
pub use platform_ops::PlatformOpsRepo;
pub use runs::RunRepo;
pub use team_members::{MemberRepo, MemberRepoError};
