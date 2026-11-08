//! ai-pm-runner 库面：Adapter（进程/标记文件）、模板库（profiles）、
//! 代码仓库域工作区准备（workspace）与 MCP 通道，供 bin 与集成测试共用。
//! 自领循环主体在 [`runner_loop`]。客户端不依赖服务端应用栈（pm-application
//! 依赖已摘除——检查点 2 §2.1 清理）。

pub mod adapter;
pub mod mcp;
pub mod profiles;
pub mod runner_loop;
pub mod workspace;
