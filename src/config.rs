//! 凭据来源：环境变量优先，其次配置文件。
//!
//! 不把 token 写进命令行参数——那会留在 shell history 和进程列表里。

use std::path::PathBuf;

/// 运行所需的凭据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    /// 爱发电后台 API token。
    pub token: String,
    /// 创作者 user_id。
    pub user_id: String,
}

/// 凭据缺失的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialError {
    /// 两个来源都没找到。
    Missing,
    /// 文件存在但格式不对。
    Malformed(String),
}

impl core::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CredentialError::Missing => write!(
                f,
                "未找到凭据。请设置 AFDIAN_TOKEN 和 AFDIAN_USER_ID 环境变量，\
                 或写入 ~/.config/afdian-cli/config.toml"
            ),
            CredentialError::Malformed(m) => write!(f, "凭据文件格式错误：{m}"),
        }
    }
}

impl std::error::Error for CredentialError {}

/// 配置文件默认位置。
pub fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join("afdian-cli").join("config.toml"))
}

/// 解析极简 TOML：只认 `key = "value"`，够用且零依赖。
pub fn parse_simple_toml(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('[') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
        if !key.is_empty() && !val.is_empty() {
            out.push((key, val));
        }
    }
    out
}

/// 从文本里取凭据。
pub fn from_text(text: &str) -> Result<Credentials, CredentialError> {
    let pairs = parse_simple_toml(text);
    let find = |names: &[&str]| {
        pairs
            .iter()
            .find(|(k, _)| names.contains(&k.as_str()))
            .map(|(_, v)| v.clone())
    };
    let token = find(&["token", "afdian_token", "api_token"]);
    let user_id = find(&["user_id", "afdian_user_id", "userid"]);
    match (token, user_id) {
        (Some(t), Some(u)) => Ok(Credentials { token: t, user_id: u }),
        _ => Err(CredentialError::Malformed(
            "需要同时提供 token 与 user_id".into(),
        )),
    }
}

/// 按「环境变量 → 配置文件」顺序解析。
pub fn resolve() -> Result<Credentials, CredentialError> {
    let env_token = std::env::var("AFDIAN_TOKEN").ok().filter(|s| !s.is_empty());
    let env_user = std::env::var("AFDIAN_USER_ID").ok().filter(|s| !s.is_empty());
    if let (Some(token), Some(user_id)) = (env_token, env_user) {
        return Ok(Credentials { token, user_id });
    }

    let path = config_path().ok_or(CredentialError::Missing)?;
    let text = std::fs::read_to_string(&path).map_err(|_| CredentialError::Missing)?;
    from_text(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_token_and_user() {
        let c = from_text(
            r#"
            # 爱发电
            token = "abc123"
            user_id = "deadbeef"
            "#,
        )
        .unwrap();
        assert_eq!(c.token, "abc123");
        assert_eq!(c.user_id, "deadbeef");
    }

    #[test]
    fn accepts_alias_keys() {
        let c = from_text("afdian_token = 'x'\nafdian_user_id = 'y'\n").unwrap();
        assert_eq!(c.token, "x");
        assert_eq!(c.user_id, "y");
    }

    #[test]
    fn rejects_partial() {
        assert!(from_text("token = \"only\"\n").is_err());
        assert!(from_text("").is_err());
    }

    #[test]
    fn ignores_comments_and_blank() {
        let pairs = parse_simple_toml("# c\n\n  a = 1  # trailing\n");
        assert_eq!(pairs, vec![("a".to_string(), "1".to_string())]);
    }
}
