//! CLI 入口。只做三件事：读参数、跑命令、写输出。

use std::process::ExitCode;

use afdian_cli::{execute_with, parse_with_format};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    let (cmd, format) = match parse_with_format(&argv) {
        Ok(c) => c,
        Err(e) => {
            // 参数错误也要遵守 --json，否则脚本无法解析失败原因。
            let json = argv.iter().any(|a| a == "--json");
            if json {
                eprintln!(
                    r#"{{"ok":false,"error":"bad_arguments","message":"{}"}}"#,
                    escape_json(&e.to_string())
                );
            } else {
                eprintln!("参数错误：{e}\n");
                eprintln!("{}", afdian_cli::commands::HELP);
            }
            return ExitCode::from(64);
        }
    };

    let outcome = execute_with(&cmd, format, |_p| {
        // 进度默认不打印，保持输出可管道化。
    });

    if outcome.code == 0 {
        println!("{}", outcome.text);
        ExitCode::SUCCESS
    } else {
        eprintln!("{}", outcome.text);
        ExitCode::from(outcome.code as u8)
    }
}

/// 只处理 JSON 必需的转义。不引 serde 是为了让入口保持零依赖。
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
