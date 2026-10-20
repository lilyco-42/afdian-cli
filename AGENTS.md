# AGENTS.md

给在本仓库工作的 AI agent 和人类的硬性规则。

## 禁止本地编译与运行

**不要**在本机执行以下任何命令：

```bash
cargo build
cargo run
cargo test
cargo clippy
cargo fmt          # 允许读，不允许改后不验证
```

原因：

1. 本机没有爱发电的凭据，跑了也是失败，只会产生噪音。
2. `cargo build` 会往 `~/.cargo` 和 `target/` 写几百 MB，污染工作区。
3. 构建结果必须可复现，只有 CI 环境是权威的。

**唯一允许的本地动作**：用 `read` / `grep` / `glob` 看代码、用 `edit` / `write` 改代码、用 `git` 提交推送。

## 验证走 CI

改完推上去，等 Actions 绿了再说「完成」。

```bash
git add -A
git commit -m "..."
git push
gh run watch
```

CI 会跑格式、clippy、三平台测试、四目标编译。任何一步红都不算完成。

## 看构建结果

```bash
gh run list --limit 3
gh run view <run-id> --log-failed     # 只看失败的部分
```

产物在 Actions 的 artifacts 里，或打 tag 后进 Releases。

## 代码约定

- 每个模块自带 `#[cfg(test)] mod tests`，不写空测试。
- 错误类型实现 `Display` + `Error`，不 `unwrap()` 在库代码里。
- 不引新的加密 / HTTP 依赖，除非有明确理由写进注释。
- 中文注释，面向读代码的人，不复述代码本身。

## 不要做的事

- 不要把 token、user_id 或任何凭据写进仓库。
- 不要在 `README` 里放真实凭据的示例值（用 `你的token` 这种占位）。
- 不要为了「跑通」而临时加 `#[ignore]` 或注释掉测试。
