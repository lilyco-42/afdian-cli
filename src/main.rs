//! CLI 入口。只做三件事：读参数、跑命令、写输出。

use std::process::ExitCode;

use afdian_cli::{commands, execute, parse};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    let cmd = match parse(&argv) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("参数错误：{e}\n");
            eprintln!("{}", commands::HELP);
            return ExitCode::from(64);
        }
    };

    let outcome = execute(&cmd, |_p| {
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
