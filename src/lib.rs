//! 爱发电 CLI —— 用 `tiny_lilyco` 的声明式骨架对接爱发电开放接口。
//!
//! 设计取舍：
//!
//! * **密码学来自 `lilyco-crypto`**。MD5 与常量时间比较不自己实现，
//!   见该仓库的 `AGENTS.md`。
//! * **凭据只走环境变量或文件**。命令行参数会留在 shell history 和进程列表里。
//! * **紧凑 JSON 自己拼**。`serde_json` 默认转义非 ASCII，而爱发电的签名
//!   要求中文原样参与计算。

pub mod api;
pub mod commands;
pub mod config;

pub use commands::{execute, parse, Command, HELP};
pub use config::Credentials;
