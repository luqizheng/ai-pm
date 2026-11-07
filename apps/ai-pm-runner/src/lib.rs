//! ai-pm-runner 库面：Adapter（进程/标记文件）与 MCP 通道，供 bin 与集成
//! 测试共用。自领循环主体在 bin（main.rs）。

pub mod adapter;
pub mod mcp;
pub mod runner_loop;
