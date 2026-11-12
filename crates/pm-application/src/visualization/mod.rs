//! Visualization Engine 应用层：词表加载与图形输入指纹。
//!
//! 定位（ADR-VIZ-001）：共享基础设施，不是独立业务域。本模块**只读**业务域数据，
//! 不提供任何业务写路径（AC-16：接受图形不改变业务事实）。
//!
//! 分层：
//! - [`registry_loader`]：Relation Registry 加载与结构校验（fail-fast）。
//! - [`source_hash`]：图形输入指纹（stale 判定依据）。

pub mod registry_loader;
pub mod source_hash;

pub use registry_loader::{load_registry, load_registry_from_path, parse_registry, REGISTRY_FILE};
pub use source_hash::source_hash;
