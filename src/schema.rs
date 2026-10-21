//! 命令声明：与 `tiny_lilyco` 的 `TinyApp` 骨架同构，但不引依赖。
//!
//! 上游 `tiny_lilyco` 只有 `CommandSchema`（名字 + 参数上限）和 `Progress`
//! 两个类型是本 CLI 用到的。它们各只有几行，抄进来比背一个 git 依赖更划算：
//!
//! * crates.io 不接受 git 依赖，引了就没法 `cargo binstall`。
//! * 上游没有发布到 crates.io，没法用版本号引。
//!
//! 声明式的形状保持一致，将来上游发布了可以直接换回去。

/// 一个命令的声明式描述。名字即 MCP 工具名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSchema {
    /// 稳定命令名，用于分发与 MCP 工具名。
    pub name: &'static str,
    /// 接受的最大参数长度（字节）。
    pub max_len: usize,
}

impl CommandSchema {
    /// 构造。
    pub const fn new(name: &'static str, max_len: usize) -> Self {
        CommandSchema { name, max_len }
    }

    /// CLI / TUI / Web / MCP 共用的输入校验。
    ///
    /// 拒绝空输入、超长输入与控制字符。非 ASCII 放行，中文参数原样通过。
    pub fn validate_input(&self, input: &str) -> Result<(), &'static str> {
        if input.is_empty() {
            return Err("input cannot be empty");
        }
        if input.len() > self.max_len {
            return Err("input exceeds max length");
        }
        let bad = input
            .bytes()
            .any(|b| b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r');
        if bad {
            return Err("input contains control characters");
        }
        Ok(())
    }
}

/// 执行阶段。四端共用同一套状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// 开始。
    Init,
    /// 进行中。
    Running {
        /// 已完成数量。
        current: u32,
        /// 总数量。
        total: u32,
    },
    /// 完成。
    Done,
    /// 失败。
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_overlong() {
        let s = CommandSchema::new("demo", 4);
        assert!(s.validate_input("").is_err());
        assert!(s.validate_input("toolong").is_err());
        assert!(s.validate_input("ok").is_ok());
    }

    #[test]
    fn allows_cjk_and_rejects_control_chars() {
        let s = CommandSchema::new("demo", 64);
        assert!(s.validate_input("中文参数").is_ok());
        assert!(s.validate_input("bad\u{0007}").is_err());
        // 制表与换行是允许的
        assert!(s.validate_input("a\tb\nc").is_ok());
    }

    #[test]
    fn progress_variants_are_comparable() {
        assert_eq!(Progress::Init, Progress::Init);
        assert_ne!(Progress::Init, Progress::Done);
        assert_eq!(
            Progress::Running {
                current: 1,
                total: 2
            },
            Progress::Running {
                current: 1,
                total: 2
            }
        );
    }
}
