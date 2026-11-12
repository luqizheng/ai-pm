//! Visualization Engine 的领域模型（V0.1）：VizSpec 类型、来源引用、关系词表与校验器。
//!
//! 定位（ADR-VIZ-001）：Visualization 是**共享基础设施**，不是独立业务域。
//! 本模块只持有**呈现层**的类型与校验规则，不持有任何业务事实——节点与边的
//! 语义权威在各自业务领域（delivery / design / pc / kb / ai）。
//!
//! 分层：
//! - [`spec`]：四段式 VizSpec（view / nodes / edges / meta），零渲染器专属字段。
//! - [`source_ref`]：来源引用与间接推导标记（与 `kb.memory_ref` 同构）。
//! - [`nature`]：事实性质维度（business / explanatory）。
//! - [`registry`]：Relation Registry 数据结构与治理规则（映射与校验，非权威）。
//! - [`validation`]：纯函数校验器与分级问题列表。
//!
//! 依赖方向：本模块位于 pm-domain 最底层——不依赖 DB、不依赖应用层；
//! 需要 DB 判定的部分由应用层实现 `validation::ObjectResolver` 注入。

pub mod nature;
pub mod registry;
pub mod source_ref;
pub mod spec;
pub mod validation;

pub use nature::Nature;
pub use registry::{
    validate_registry, NodeKindDefinition, RelationDefinition, RelationRegistry, RelationStatus,
    RegistryError,
};
pub use source_ref::{DerivedTrace, SourceRef};
pub use spec::{
    Budget, Provenance, SpecLimits, VizEdge, VizLens, VizMeta, VizNode, VizSpec, VizView,
    SPEC_VERSION,
};
pub use validation::{
    codes, validate, ObjectResolver, Severity, ValidationContext, ValidationReport, VizProblem,
};
