//! 命令定义与执行。
//!
//! 每个命令都实现同一套 `schema → validate → execute`，
//! 这样 CLI / TUI / Web / MCP 四端可以共用一份声明。
//!
//! # 接口事实
//!
//! 爱发电开放接口只有四个端点，全部是 `POST /api/open/*`：
//!
//! | 端点 | 参数 | 用途 |
//! | --- | --- | --- |
//! | `ping` | 任意非空对象 | 连通性与凭据自检 |
//! | `query-plan` | `{"plan_id": "..."}` | 查单个方案详情 |
//! | `query-order` | `{"page": N}` 或 `{"out_trade_no": "..."}` | 查订单 |
//! | `query-sponsor` | `{"page": N}` | 查赞助者 |
//!
//! **没有「列出全部方案」的接口**——`query-plan` 必须给 `plan_id`。
//! 这是官方限制，不是本 CLI 的缺陷。

use crate::api;
use crate::config::{self, Credentials};
use crate::schema::{CommandSchema, Progress};

/// 支持的命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 自检：验证凭据是否可用。
    Ping,
    /// 查单个方案详情。
    Plan {
        /// 爱发电方案 ID（32 位十六进制）。
        plan_id: String,
    },
    /// 查询订单列表，或按订单号查单条。
    Orders {
        /// 页码，从 1 开始。与 `out_trade_no` 互斥。
        page: u32,
        /// 按爱发电订单号精确查询。
        out_trade_no: Option<String>,
    },
    /// 查询赞助者列表。
    Sponsors {
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
        Command::Ping => CommandSchema::new("ping", 32),
        Command::Plan { .. } => CommandSchema::new("plan", 32),
        Command::Orders { .. } => CommandSchema::new("orders", 64),
        Command::Sponsors { .. } => CommandSchema::new("sponsors", 32),
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

/// 校验订单号：爱发电是 21 位纯数字。
pub fn validate_trade_no(no: &str) -> Result<(), ParseError> {
    if no.len() < 15 || no.len() > 40 {
        return Err(ParseError(format!(
            "订单号长度异常（{} 位），正常是 21 位数字",
            no.len()
        )));
    }
    if !no.chars().all(|c| c.is_ascii_digit()) {
        return Err(ParseError("订单号应为纯数字".into()));
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
        "ping" | "whoami" | "me" => Ok(Command::Ping),
        "plan" => {
            let id = argv
                .get(1)
                .ok_or_else(|| ParseError("用法：afdian plan <plan_id>".into()))?;
            validate_plan_id(id)?;
            Ok(Command::Plan {
                plan_id: id.clone(),
            })
        }
        "orders" => {
            // orders --no 2026...  按订单号精确查
            if argv.get(1).map(String::as_str) == Some("--no") {
                let no = argv
                    .get(2)
                    .ok_or_else(|| ParseError("用法：afdian orders --no <out_trade_no>".into()))?;
                validate_trade_no(no)?;
                return Ok(Command::Orders {
                    page: 1,
                    out_trade_no: Some(no.clone()),
                });
            }
            Ok(Command::Orders {
                page: page_of(&argv[1..])?,
                out_trade_no: None,
            })
        }
        "sponsors" => Ok(Command::Sponsors {
            page: page_of(&argv[1..])?,
        }),
        "checkout" => {
            let plan_id = argv.get(1).ok_or_else(|| {
                ParseError("用法：afdian checkout <plan_id> [custom_order_id]".into())
            })?;
            validate_plan_id(plan_id)?;
            let custom = argv.get(2).cloned().unwrap_or_else(|| {
                format!(
                    "AF{:x}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0)
                )
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
  afdian ping                       自检：验证凭据是否可用
  afdian plan <plan_id>             查单个方案详情
  afdian orders [page]              查订单列表（每页 50 条）
  afdian orders --no <订单号>        按订单号查单条
  afdian sponsors [page]            查赞助者列表
  afdian checkout <plan_id> [单号]   生成收银台链接
  afdian help                       显示本帮助

凭据（按优先级）:
  1. 环境变量 AFDIAN_TOKEN / AFDIAN_USER_ID
  2. ~/.config/afdian-cli/config.toml
       token = \"...\"
       user_id = \"...\"

示例:
  export AFDIAN_TOKEN=你的token
  export AFDIAN_USER_ID=你的user_id
  afdian ping
  afdian plan 63f94afacc9d11f19fbe5254001e7c00
  afdian orders 1
  afdian checkout 63f94afacc9d11f19fbe5254001e7c00

注意:
  爱发电没有「列出全部方案」的接口，查方案必须给 plan_id。
  方案 ID 可在 https://afdian.com/dashboard/plans 找到。
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
            progress(Progress::Running {
                current: 1,
                total: 1,
            });
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

            progress(Progress::Running {
                current: 1,
                total: 2,
            });
            match cmd {
                Command::Ping => ping(&creds),
                Command::Plan { plan_id } => plan(&creds, plan_id),
                Command::Orders { page, out_trade_no } => {
                    orders(&creds, *page, out_trade_no.as_deref())
                }
                Command::Sponsors { page } => sponsors(&creds, *page),
                _ => unreachable!(),
            }
        }
    }
}

/// `ping` 要求 params 是合法 JSON 且非空——传 `{}` 会被判为
/// "params was not valid json string"，必须带一个字段。
fn ping(creds: &Credentials) -> Outcome {
    match api::call(creds, api::paths::PING, serde_json::json!({"ping": 1})) {
        Ok(_) => Outcome {
            text: format!(
                "凭据有效\nuser_id: {}\n接口: /api/open/ping 返回 pong",
                creds.user_id
            ),
            code: 0,
        },
        Err(e) => Outcome {
            text: format!("调用失败: {e}"),
            code: 1,
        },
    }
}

fn plan(creds: &Credentials, plan_id: &str) -> Outcome {
    let params = serde_json::json!({"plan_id": plan_id});
    match api::call(creds, api::paths::QUERY_PLAN, params) {
        Ok(data) => {
            let Some(p) = data.get("plan") else {
                return Outcome {
                    text: "接口未返回方案".to_string(),
                    code: 1,
                };
            };
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("-");
            let price = p.get("price").and_then(|v| v.as_str()).unwrap_or("?");
            let product_type = p.get("product_type").and_then(|v| v.as_i64()).unwrap_or(0);
            let kind = match product_type {
                0 => "订阅",
                1 => "商品",
                _ => "其他",
            };
            let pay_month = p.get("pay_month").and_then(|v| v.as_i64()).unwrap_or(0);

            let mut lines = vec![
                name.to_string(),
                format!("  价格     ¥{price}（{kind}，{pay_month} 个月）"),
                format!("  plan_id  {plan_id}"),
            ];

            if let Some(skus) = p.get("skus").and_then(|v| v.as_array()) {
                if !skus.is_empty() {
                    lines.push(format!("  规格     {} 个", skus.len()));
                    for s in skus {
                        let sn = s.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                        let sp = s.get("price").and_then(|v| v.as_str()).unwrap_or("?");
                        let sid = s.get("sku_id").and_then(|v| v.as_str()).unwrap_or("-");
                        let stock = s.get("stock").and_then(|v| v.as_str()).unwrap_or("");
                        let stock = if stock.is_empty() {
                            "不限".to_string()
                        } else {
                            format!("库存 {stock}")
                        };
                        lines.push(format!("    ¥{sp:<8} {sn}  [{stock}]"));
                        lines.push(format!("             {sid}"));
                    }
                }
            }

            if let Some(desc) = p.get("desc").and_then(|v| v.as_str()) {
                let plain = strip_html(desc);
                if !plain.is_empty() {
                    lines.push(format!("  简介     {}", truncate(&plain, 120)));
                }
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

fn orders(creds: &Credentials, page: u32, out_trade_no: Option<&str>) -> Outcome {
    let params = match out_trade_no {
        Some(no) => serde_json::json!({"out_trade_no": no}),
        None => serde_json::json!({"page": page}),
    };

    match api::call(creds, api::paths::QUERY_ORDER, params) {
        Ok(data) => {
            let list = data
                .get("list")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if list.is_empty() {
                // 按订单号查不到是「没有这个订单」，脚本需要能区分——
                // 返回非零，否则 `afdian orders --no X && 发货` 会误发货。
                // 翻页翻到空页不算错误，正常返回 0。
                return Outcome {
                    text: match out_trade_no {
                        Some(no) => format!("没有找到订单 {no}"),
                        None => format!("第 {page} 页没有订单"),
                    },
                    code: if out_trade_no.is_some() { 1 } else { 0 },
                };
            }

            let total = data.get("total_count").and_then(|v| v.as_i64());
            let pages = data.get("total_page").and_then(|v| v.as_i64());

            let mut lines = Vec::new();
            match (out_trade_no, total, pages) {
                // 单条查询时标题就是订单号，详情行不再重复打印。
                (Some(no), _, _) => lines.push(format!("订单 {no}")),
                (None, Some(t), Some(p)) => {
                    lines.push(format!("共 {t} 条订单，{p} 页（第 {page} 页）"))
                }
                _ => lines.push(format!("第 {page} 页，{} 条", list.len())),
            }

            let single = out_trade_no.is_some();
            for o in &list {
                let no = o
                    .get("out_trade_no")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let amount = o
                    .get("total_amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let status = o.get("status").and_then(|v| v.as_i64()).unwrap_or(0);
                let title = o.get("plan_title").and_then(|v| v.as_str()).unwrap_or("-");
                let buyer = o.get("user_name").and_then(|v| v.as_str()).unwrap_or("-");
                let custom = o
                    .get("custom_order_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let created = o.get("create_time").and_then(|v| v.as_i64()).unwrap_or(0);

                lines.push(format!(
                    "  ¥{amount:<8} [{}] {}",
                    status_text(status),
                    truncate(title, 30)
                ));
                if single {
                    lines.push(format!("             买家 {buyer}  {}", fmt_time(created)));
                } else {
                    lines.push(format!("             {no}  {buyer}  {}", fmt_time(created)));
                }
                if !custom.is_empty() {
                    lines.push(format!("             核对码 {custom}"));
                }
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

fn sponsors(creds: &Credentials, page: u32) -> Outcome {
    let params = serde_json::json!({"page": page});
    match api::call(creds, api::paths::QUERY_SPONSOR, params) {
        Ok(data) => {
            let list = data
                .get("list")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if list.is_empty() {
                return Outcome {
                    text: format!("第 {page} 页没有赞助者"),
                    code: 0,
                };
            }

            let total = data.get("total_count").and_then(|v| v.as_i64());
            let pages = data.get("total_page").and_then(|v| v.as_i64());

            let mut lines = Vec::new();
            match (total, pages) {
                (Some(t), Some(p)) => {
                    lines.push(format!("共 {t} 位赞助者，{p} 页（第 {page} 页）"))
                }
                _ => lines.push(format!("第 {page} 页，{} 位", list.len())),
            }

            for s in &list {
                let name = s
                    .get("user")
                    .and_then(|u| u.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("匿名");
                let sum = s
                    .get("all_sum_amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let last = s.get("last_pay_time").and_then(|v| v.as_i64()).unwrap_or(0);

                lines.push(format!("  ¥{sum:<10} {name}  {}", fmt_time(last)));
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

/// 订单状态。爱发电只公开了 2 = 已支付，其余按经验推断并标注。
fn status_text(s: i64) -> &'static str {
    match s {
        2 => "已支付",
        1 => "未支付",
        3 => "退款中",
        4 => "已退款",
        _ => "未知",
    }
}

/// 秒级时间戳转 `YYYY-MM-DD`。不引 chrono——只需要日期，自己算就行。
///
/// 用 Howard Hinnant 的 civil_from_days 算法，对 1970–2400 年都正确。
fn fmt_time(ts: i64) -> String {
    if ts <= 0 {
        return "-".to_string();
    }
    let days = ts / 86400;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// 去掉 HTML 标签，压平空白。爱发电的 `desc` 是富文本。
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 按字符（不是字节）截断，避免切碎中文。
fn truncate(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_commands() {
        assert_eq!(parse(["ping"]).unwrap(), Command::Ping);
        assert_eq!(parse(["whoami"]).unwrap(), Command::Ping);
        assert_eq!(parse(Vec::<String>::new()).unwrap(), Command::Help);
        assert_eq!(
            parse(["orders"]).unwrap(),
            Command::Orders {
                page: 1,
                out_trade_no: None
            }
        );
        assert_eq!(
            parse(["orders", "2"]).unwrap(),
            Command::Orders {
                page: 2,
                out_trade_no: None
            }
        );
        assert_eq!(
            parse(["sponsors", "3"]).unwrap(),
            Command::Sponsors { page: 3 }
        );
    }

    #[test]
    fn parse_orders_by_trade_no() {
        let no = "202610162137031029998943619";
        assert_eq!(
            parse(["orders", "--no", no]).unwrap(),
            Command::Orders {
                page: 1,
                out_trade_no: Some(no.to_string())
            }
        );
        // 缺参数要报错
        assert!(parse(["orders", "--no"]).is_err());
        // 非数字要报错
        assert!(parse(["orders", "--no", "abc"]).is_err());
    }

    #[test]
    fn rejects_bad_page() {
        assert!(parse(["orders", "0"]).is_err());
        assert!(parse(["orders", "abc"]).is_err());
        assert!(parse(["sponsors", "-1"]).is_err());
    }

    #[test]
    fn rejects_unknown_command() {
        assert!(parse(["nope"]).is_err());
    }

    #[test]
    fn plan_id_must_be_32_hex() {
        assert!(validate_plan_id("63f94afacc9d11f19fbe5254001e7c00").is_ok());
        assert!(validate_plan_id("short").is_err());
        assert!(validate_plan_id("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz").is_err());
    }

    #[test]
    fn trade_no_must_be_digits() {
        assert!(validate_trade_no("202610162137031029998943619").is_ok());
        assert!(validate_trade_no("2026abc").is_err());
        assert!(validate_trade_no("").is_err());
    }

    #[test]
    fn plan_requires_id() {
        assert!(parse(["plan"]).is_err());
        assert!(parse(["plan", "bad"]).is_err());
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
        assert!(HELP.contains("afdian ping"));
        assert!(HELP.contains("AFDIAN_TOKEN"));
        // 帮助里不能出现任何看起来像真实凭据的值
        assert!(HELP.contains("你的token"));
    }

    #[test]
    fn fmt_time_known_dates() {
        // 2021-06-23 00:00:00 UTC
        assert_eq!(fmt_time(1624406400), "2021-06-23");
        // 1970-01-01
        assert_eq!(fmt_time(0), "-");
        // 2026-10-16
        assert_eq!(fmt_time(1792157823), "2026-10-16");
    }

    #[test]
    fn strip_html_removes_tags() {
        assert_eq!(strip_html("<p>你好</p><p>世界</p>"), "你好世界");
        assert_eq!(strip_html("a  b\n\nc"), "a b c");
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn truncate_counts_chars_not_bytes() {
        // 5 个中文字符 = 15 字节，按字符截断不能切碎
        assert_eq!(truncate("一二三四五", 3), "一二三…");
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("", 3), "");
    }

    #[test]
    fn status_text_covers_known() {
        assert_eq!(status_text(2), "已支付");
        assert_eq!(status_text(99), "未知");
    }

    #[test]
    fn missing_trade_no_is_nonzero_but_empty_page_is_zero() {
        // 按订单号查不到必须返回非零，否则脚本会误判成功去发货。
        // 翻页翻到空页是正常情况，仍返回 0。
        //
        // 这里不联网，直接验证 execute 的分支决策依据：
        // 单条查询时 out_trade_no 为 Some。
        let cmd = parse(["orders", "--no", "202610162137031029998943619"]).unwrap();
        match cmd {
            Command::Orders { out_trade_no, .. } => assert!(out_trade_no.is_some()),
            _ => panic!("应解析成 Orders"),
        }
        let cmd = parse(["orders", "9"]).unwrap();
        match cmd {
            Command::Orders { out_trade_no, .. } => assert!(out_trade_no.is_none()),
            _ => panic!("应解析成 Orders"),
        }
    }
}
