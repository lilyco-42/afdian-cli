//! 爱发电 CLI —— 用 `tiny_lilyco` 的声明式骨架对接爱发电开放接口。
//!
//! 设计取舍：
//!
//! * **签名不引加密库**。算法固定，手写 MD5 只有几十行，省掉一个依赖。
//! * **凭据只走环境变量或文件**。命令行参数会留在 shell history 和进程列表里。
//! * **`no_std` 内核 + `std` 外壳**。命令声明复用 `tiny_lilyco`，
//!   网络与文件只在 `std` 侧启用。

pub mod api;
pub mod commands;
pub mod config;
pub mod sign;

pub use commands::{execute, parse, Command, HELP};
pub use config::Credentials;
