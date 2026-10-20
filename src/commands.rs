//! 命令定义与执行。
//!
//! 每个命令都实现同一套 `schema → validate → execute`，
//! 这样 CLI / TUI / Web / MCP 四端可以共用一份声明。

use crate::api;
use crate::config::{self, Credentials};
use tiny_lilyco::prelude::{CommandSchema, Progress};

/// 支持的命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 查看账户信息，验证凭据是否可用。
    Whoami,
    /// 列出已上架的方案 / 商品。
    Plans {
        /// 页码，从 1 开始。
        page: u32,
    },
    /// 查询订单列表。
    Orders {
        /// 页码，从 1 开始。
        page: u32,
    },
    /// 生成收银台链接。
    Checkout {
        /// 爱发电方案 ID（32 位十六进制）。
        plan_id: String,
        /// 自定义订单号，用于回跳关联。
        custom_order_id: String,
    },
    /// 打印帮助。
    Help,
}

/// 解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// 各命令的 schema。名字即 MCP 工具名。
pub fn schema_for(cmd: &Command) -> CommandSchema {
    match cmd {
        Command::Whoami => CommandSchema::new("whoami", 32),
        Command::Plans { .. } => CommandSchema::new("plans", 32),
        Command::Orders { .. } => CommandSchema::new("orders", 32),
        Command::Checkout { .. } => CommandSchema::new("checkout", 256),
        Command::Help => CommandSchema::new("help", 32),
    }
}

/// 校验方案 ID：爱发电固定 32 位小写十六进制。
pub fn validate_plan_id(id: &str) -> Result<(), ParseError> {
    if id.len() != 32 {
        return Err(ParseError(format!(
            "方案 ID 应为 32 位十六进制，实际 {} 位",
            id.len()
        )));
    }
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ParseError("方案 ID 含非十六进制字符".into()));
    }
    Ok(())
}

/// 解析命令行参数。
pub fn parse<I, S>(args: I) -> Result<Command, ParseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let argv: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    let Some(head) = argv.first().map(String::as_str) else {
        return Ok(Command::Help);
    };

    let page_of = |rest: &[String]| -> Result<u32, ParseError> {
        match rest.first() {
            None => Ok(1),
            Some(raw) => raw
                .parse::<u32>()
                .map_err(|_| ParseError(format!("页码必须是正整数，收到 {raw}")))
                .and_then(|p| {
                    if p == 0 {
                        Err(ParseError("页码从 1 开始".into()))
                    } else {
                        Ok(p)
                    }
                }),
        }
    };

    match head {
        "whoami" | "me" => Ok(Command::Whoami),
        "plans" => Ok(Command::Plans {
            page: page_of(&argv[1..])?,
        }),
        "orders" => Ok(Command::Orders {
            page: page_of(&argv[1..])?,
        }),
        "checkout" => {
            let plan_id = argv
                .get(1)
                .ok_or_else(|| ParseError("用法：afdian checkout <plan_id> [custom_order_id]".into()))?;
            validate_plan_id(plan_id)?;
            let custom = argv.get(2).cloned().unwrap_or_else(|| {
                format!("AF{:x}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0))
            });
            Ok(Command::Checkout {
                plan_id: plan_id.clone(),
                custom_order_id: custom,
            })
        }
        "help" | "-h" | "--help" => Ok(Command::Help),
        other => Err(ParseError(format!("未知命令：{other}"))),
    }
}

/// 收银台链接。`custom_order_id` 会原样带回，用于关联本站订单。
pub fn checkout_url(plan_id: &str, custom_order_id: &str) -> String {
    format!(
        "https://afdian.com/order/create?plan_id={plan_id}&product_type=1&month=1&custom_order_id={custom_order_id}"
    )
}

/// 执行结果。
pub struct Outcome {
    /// 输出到 stdout 的文本。
    pub text: String,
    /// 进程退出码。
    pub code: i32,
}

/// 帮助文本。
pub const HELP: &str = "\
afdian — 爱发电开放接口 CLI

用法:
  afdian whoami                     查看账户信息，验证凭据
  afdian plans [page]               列出已上架方案 / 商品
  afdian orders [page]              查询订单列表
  afdian checkout <plan_id> [单号]   生成收银台链接
  afdian help                       显示本帮助

凭据（按优先级）:
  1. 环境变量 AFDIAN_TOKEN / AFDIAN_USER_ID
  2. ~/.config/afdian-cli/config.toml
       token = \"...\"
       user_id = \"...\"

示例:
  export AFDIAN_TOKEN=xxx
  export AFDIAN_USER_ID=yyy
  afdian whoami
  afdian plans
  afdian checkout 0e65b484c2d411f192e852540025c377
";

/// 执行命令。`progress` 用于上报阶段，四端共用。
pub fn execute<F>(cmd: &Command, mut progress: F) -> Outcome
where
    F: FnMut(Progress),
{
    match cmd {
        Command::Help => Outcome {
            text: HELP.to_string(),
            code: 0,
        },
        Command::Checkout {
            plan_id,
            custom_order_id,
        } => {
            progress(Progress::Running { current: 1, total: 1 });
            Outcome {
                text: format!(
                    "{}\n\n把用户跳转到上面的链接即可。custom_order_id={}",
                    checkout_url(plan_id, custom_order_id),
                    custom_order_id
                ),
                code: 0,
            }
        }
        _ => {
            progress(Progress::Init);
            let creds = match config::resolve() {
                Ok(c) => c,
                Err(e) => {
                    return Outcome {
                        text: format!("{e}"),
                        code: 2,
                    }
                }
            };

            progress(Progress::Running { current: 1, total: 2 });
            match cmd {
                Command::Whoami => whoami(&creds),
                Command::Plans { page } => plans(&creds, *page),
                Command::Orders { page } => orders(&creds, *page),
                _ => unreachable!(),
            }
        }
    }
}

fn whoami(creds: &Credentials) -> Outcome {
    match api::call(creds, "/api/user/get-profile", serde_json::json!({})) {
        Ok(data) => {
            let name = data
                .get("user")
                .and_then(|u| u.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("(未返回昵称)");
            Outcome {
                text: format!("凭据有效\n昵称: {name}\nuser_id: {}", creds.user_id),
                code: 0,
            }
        }
        Err(e) => Outcome {
            text: format!("调用失败: {e}"),
            code: 1,
        },
    }
}

fn plans(creds: &Credentials, page: u32) -> Outcome {
    let params = serde_json::json!({"page": page});
    match api::call(creds, "/api/creator/all-plans", params) {
        Ok(data) => {
            let mut lines = Vec::new();
            if let Some(list) = data.get("list").and_then(|v| v.as_array()) {
                lines.push(format!("共 {} 个方案（第 {page} 页）", list.len()));
                for p in list {
                    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                    let price = p.get("price").and_then(|v| v.as_str()).unwrap_or("?");
                    let id = p.get("plan_id").and_then(|v| v.as_str()).unwrap_or("-");
                    let status = p.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
                    let mark = if status == 1 { "上架" } else { "下架" };
                    lines.push(format!("  [{mark}] ¥{price:<8} {name}\n           {id}"));
                }
            } else {
                lines.push("接口未返回方案列表".to_string());
            }
            Outcome {
                text: lines.join("\n"),
                code: 0,
            }
        }
        Err(e) => Outcome {
            text: format!("调用失败: {e}"),
            code: 1,
        },
    }
}

fn orders(creds: &Credentials, page: u32) -> Outcome {
    let params = serde_json::json!({"page": page});
    match api::call(creds, "/api/creator/list-order", params) {
        Ok(data) => {
            let mut lines = Vec::new();
            if let Some(list) = data.get("list").and_then(|v| v.as_array()) {
                let count = data
                    .get("count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(list.len() as i64);
                lines.push(format!("共 {count} 条订单（第 {page} 页）"));
                for o in list {
                    let no = o.get("out_trade_no").and_then(|v| v.as_str()).unwrap_or("-");
                    let amount = o.get("total_amount").and_then(|v| v.as_str()).unwrap_or("?");
                    let custom = o.get("custom_order_id").and_then(|v| v.as_str()).unwrap_or("");
                    lines.push(format!(
                        "  ¥{amount:<8} {no}{}",
                        if custom.is_empty() {
                            String::new()
                        } else {
                            format!("  ← {custom}")
                        }
                    ));
                }
            } else {
                lines.push("接口未返回订单列表".to_string());
            }
            Outcome {
                text: lines.join("\n"),
                code: 0,
            }
        }
        Err(e) => Outcome {
            text: format!("调用失败: {e}"),
            code: 1,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_commands() {
        assert_eq!(parse(["whoami"]).unwrap(), Command::Whoami);
        assert_eq!(parse(["me"]).unwrap(), Command::Whoami);
        assert_eq!(parse(Vec::<String>::new()).unwrap(), Command::Help);
        assert_eq!(parse(["plans"]).unwrap(), Command::Plans { page: 1 });
        assert_eq!(parse(["plans", "3"]).unwrap(), Command::Plans { page: 3 });
        assert_eq!(parse(["orders", "2"]).unwrap(), Command::Orders { page: 2 });
    }

    #[test]
    fn rejects_bad_page() {
        assert!(parse(["plans", "0"]).is_err());
        assert!(parse(["plans", "abc"]).is_err());
        assert!(parse(["plans", "-1"]).is_err());
    }

    #[test]
    fn rejects_unknown_command() {
        assert!(parse(["nope"]).is_err());
    }

    #[test]
    fn plan_id_must_be_32_hex() {
        assert!(validate_plan_id("0e65b484c2d411f192e852540025c377").is_ok());
        assert!(validate_plan_id("short").is_err());
        assert!(validate_plan_id("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz").is_err());
    }

    #[test]
    fn checkout_requires_plan_id() {
        assert!(parse(["checkout"]).is_err());
        assert!(parse(["checkout", "bad"]).is_err());
    }

    #[test]
    fn checkout_url_shape() {
        let u = checkout_url("abc", "ORD1");
        assert!(u.starts_with("https://afdian.com/order/create?"));
        assert!(u.contains("plan_id=abc"));
        assert!(u.contains("custom_order_id=ORD1"));
    }

    #[test]
    fn help_has_no_placeholder() {
        assert!(HELP.contains("afdian whoami"));
        assert!(HELP.contains("AFDIAN_TOKEN"));
    }
}

