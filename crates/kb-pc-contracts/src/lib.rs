//! KB-PC 服务化契约 crate（Phase 1，架构基准
//! `docs/architecture/knowledge-services/kb-pc-independent-services-architecture-v1.1.md`）。
//!
//! 三服务（Platform / KB-Service / PC-Service）V1 形态为进程内端口调用 +
//! 单 PG + 契约先行（ADR-10 方案 A）。本 crate 承载**跨服务共享的契约面**，
//! 是 Phase 2「契约冻结」的类型载体：
//!
//! - [`domain_ref`]：四元组跨服务引用本体（ADR-07）。**service 仅作信封级
//!   元数据**（事件顶层 `producer` / `intake_task.target_service` 列），禁止
//!   进入任何持久化引用结构——类型系统上 `DomainRef` 没有该字段。
//! - [`registry`]：domain 注册表（PC-01B §13.1 逐域通道表的操作化）：wire
//!   字符串 ↔ 枚举、归属服务、通道开放状态；解析失败返回告警错误类型。
//! - [`envelope`]：事件信封（V1.1 §13.1 十一字段）。
//! - [`events`]：14 项跨服务事件目录（V1.1 §13.3，目录=单一真源；与
//!   `pm-application::outbox::CATALOG` 的登记面共用本清单常量）。
//! - [`ports`]：KbServicePort / PcServicePort 端口 trait（V1.1 §12 契约），
//!   V1 进程内实现占位——`PortNotWired` 返回未接线错误，业务落点归 Phase 4/5。
//!
//! 红线（V1.1 §21 ADR 裁决）：不引 MQ（ADR-03，全部异步走 Transactional
//! Outbox）；payload 只含四元组引用 + 摘要（≤256 字符）+ hash，禁止内联正文、
//! 禁止内嵌 service 字段；事件 additive-only（ADR-11）。

pub mod domain_ref;
pub mod envelope;
pub mod events;
pub mod ports;
pub mod registry;

pub use domain_ref::DomainRef;
pub use envelope::EventEnvelope;
pub use ports::{KbServicePort, PcServicePort, PortError};
pub use registry::{DomainChannelState, OwningService, RegistryError, TargetDomain};
