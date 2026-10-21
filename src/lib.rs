//! 爱发电 CLI —— 声明式命令骨架，对接爱发电开放接口。
//!
//! 设计取舍：
//!
//! * **密码学来自 `lilyco-crypto`**。MD5 与常量时间比较不自己实现，
//!   见该仓库的 `AGENTS.md`。
//! * **命令声明抄自 `tiny_lilyco`**，但只内联了用到的两个类型。
//!   crates.io 不接受 git 依赖，引了就没法 `cargo binstall`。
//! * **凭据只走环境变量或文件**。命令行参数会留在 shell history 和进程列表里。
//! * **紧凑 JSON 自己拼**。`serde_json` 默认转义非 ASCII，而爱发电的签名
//!   要求中文原样参与计算。

pub mod api;
pub mod commands;
pub mod config;
pub mod schema;

pub use commands::{execute, execute_with, parse, parse_with_format, Command, Format, HELP};
pub use config::Credentials;
pub use schema::{CommandSchema, Progress};
