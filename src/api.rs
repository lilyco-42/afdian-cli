//! 爱发电开放接口客户端。
//!
//! 覆盖对接时真正会用到的四个动作：查账户、列方案、建订单、查订单。
//! 所有请求都走同一套签名，参数固定为紧凑 JSON。

use crate::config::Credentials;
use crate::sign::SigningInput;

/// 接口基址。
pub const BASE: &str = "https://afdian.com";

/// 调用失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// 网络层失败。
    Network(String),
    /// HTTP 状态非 2xx。
    Status(u16, String),
    /// 业务层 `ec != 200`。
    Biz(i64, String),
    /// 返回体不是预期 JSON。
    Decode(String),
}

impl core::fmt::Display for ApiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ApiError::Network(m) => write!(f, "网络错误：{m}"),
            ApiError::Status(c, b) => write!(f, "HTTP {c}：{b}"),
            ApiError::Biz(ec, em) => write!(f, "接口返回 ec={ec}：{em}"),
            ApiError::Decode(m) => write!(f, "响应解析失败：{m}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// 一个已签名的请求。
#[derive(Debug, Clone)]
pub struct SignedRequest {
    /// 目标路径，例如 `/api/user/get-profile`。
    pub path: String,
    /// 完整请求体。
    pub body: String,
}

/// 把参数序列化成紧凑 JSON。
///
/// 爱发电要求 `separators=(',', ':')` 且中文不转义——用 `{}` 手工拼太脆，
/// 这里用 serde_json 但关闭转义以保证与官方一致。
pub fn compact_json(value: &serde_json::Value) -> String {
    let mut out = String::new();
    write_compact(value, &mut out);
    out
}

fn write_compact(v: &serde_json::Value, out: &mut String) {
    match v {
        serde_json::Value::Object(map) => {
            // 键必须排序：签名是对参数字节序列做 MD5，顺序不稳会导致
            // 同样的参数算出不同签名。serde_json 的 Map 在未开
            // preserve_order 时是 BTreeMap（有序），但显式排序才不依赖特性开关。
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_escaped(k, out);
                out.push(':');
                if let Some(val) = map.get(k) {
                    write_compact(val, out);
                }
            }
            out.push('}');
        }
        serde_json::Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_compact(item, out);
            }
            out.push(']');
        }
        serde_json::Value::String(s) => push_escaped(s, out),
        serde_json::Value::Null => out.push_str("null"),
        other => out.push_str(&other.to_string()),
    }
}

/// 只转义 JSON 必需字符，中文原样输出。
fn push_escaped(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// 构造签名请求。
pub fn build(creds: &Credentials, path: &str, params: serde_json::Value, ts: u64) -> SignedRequest {
    let params_json = compact_json(&params);
    let sign = SigningInput {
        token: &creds.token,
        user_id: &creds.user_id,
        params: &params_json,
        ts,
    }
    .sign();

    let body = serde_json::json!({
        "user_id": creds.user_id,
        "params": params_json,
        "ts": ts,
        "sign": sign,
    });

    SignedRequest {
        path: path.to_string(),
        body: compact_json(&body),
    }
}

/// 发出请求并校验业务码。
pub fn call(
    creds: &Credentials,
    path: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let req = build(creds, path, params, ts);
    let url = format!("{BASE}{}", req.path);

    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .set(
            "User-Agent",
            "afdian-cli/0.1 (+https://github.com/lilyco-42/afdian-cli)",
        )
        .send_string(&req.body)
        .map_err(|e| match e {
            ureq::Error::Status(code, r) => {
                let body = r.into_string().unwrap_or_default();
                ApiError::Status(code, body.chars().take(300).collect())
            }
            ureq::Error::Transport(t) => ApiError::Network(t.to_string()),
        })?;

    let text = resp
        .into_string()
        .map_err(|e| ApiError::Network(e.to_string()))?;

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        ApiError::Decode(format!(
            "{e}: {}",
            text.chars().take(200).collect::<String>()
        ))
    })?;

    let ec = json.get("ec").and_then(|v| v.as_i64()).unwrap_or(-1);
    if ec != 200 {
        let em = json
            .get("em")
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误")
            .to_string();
        return Err(ApiError::Biz(ec, em));
    }

    Ok(json.get("data").cloned().unwrap_or(serde_json::Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn creds() -> Credentials {
        Credentials {
            token: "tok".into(),
            user_id: "uid".into(),
        }
    }

    #[test]
    fn compact_json_keeps_cjk_raw() {
        let s = compact_json(&json!({"name": "中文", "n": 1}));
        // 键按键名排序，中文值原样保留（不能被转义成 \uXXXX）。
        assert_eq!(s, r#"{"n":1,"name":"中文"}"#);
        assert!(s.contains("中文"));
        assert!(!s.contains("\\u"));
    }

    #[test]
    fn compact_json_key_order_is_stable() {
        // 同样内容、不同插入顺序，必须得到同样的字节序列，
        // 否则签名会随构建顺序漂移。
        let a = compact_json(&json!({"b": 2, "a": 1}));
        let b = compact_json(&json!({"a": 1, "b": 2}));
        assert_eq!(a, b);
        assert_eq!(a, r#"{"a":1,"b":2}"#);
    }

    #[test]
    fn compact_json_escapes_required_only() {
        let s = compact_json(&json!({"a": "x\"y", "b": "l\nr"}));
        assert_eq!(s, r#"{"a":"x\"y","b":"l\nr"}"#);
    }

    #[test]
    fn compact_json_nested() {
        let s = compact_json(&json!({"o": {"k": [1, 2]}}));
        assert_eq!(s, r#"{"o":{"k":[1,2]}}"#);
    }

    #[test]
    fn build_includes_all_fields() {
        let r = build(&creds(), "/api/x", json!({}), 100);
        assert!(r.body.contains(r#""user_id":"uid""#));
        assert!(r.body.contains(r#""ts":100"#));
        assert!(r.body.contains(r#""sign":"#));
        assert!(r.body.contains(r#""params":"{}""#));
    }

    #[test]
    fn sign_depends_on_params() {
        let a = build(&creds(), "/p", json!({"page": 1}), 100);
        let b = build(&creds(), "/p", json!({"page": 2}), 100);
        assert_ne!(a.body, b.body);
    }

    #[test]
    fn error_display_is_readable() {
        let e = ApiError::Biz(400, "bad params".into());
        assert!(e.to_string().contains("400"));
    }
}

