# afdian-cli

爱发电开放接口的命令行工具，基于 [`tiny_lilyco`](https://github.com/lilyco-42/lilyco) 的声明式骨架。

> **本仓库禁止本地编译与运行。** 所有构建、测试与产物都在 GitHub Actions 上完成。
> 详见 [AGENTS.md](AGENTS.md)。

## 为什么需要它

对接爱发电时，下面这几件事每次都要重写一遍：

- 签名算法（顺序错了就 403，但报错信息看不出来）
- 紧凑 JSON（`separators=(',',':')`，中文不能转义）
- 凭据怎么放（塞命令行参数会漏进 shell history）
- 方案 ID 的格式校验（32 位十六进制）

这个工具把上面四件事固化下来，一个二进制搞定。

## 安装

从 [Releases](https://github.com/lilyco-42/afdian-cli/releases) 下载对应平台：

| 平台 | 文件 |
| --- | --- |
| Linux x86_64 | `afdian-linux-x86_64` |
| Linux aarch64 | `afdian-linux-aarch64` |
| Windows x86_64 | `afdian-windows-x86_64.exe` |
| macOS Apple Silicon | `afdian-macos-arm64` |

Linux 产物是静态链接（musl），可以直接扔进 Alpine 容器。

## 凭据

**不要**把 token 写进命令行参数——那会留在 shell history 和进程列表里。用环境变量：

```bash
export AFDIAN_TOKEN=你的token
export AFDIAN_USER_ID=你的user_id
```

或者写配置文件 `~/.config/afdian-cli/config.toml`：

```toml
token = "你的token"
user_id = "你的user_id"
```

两个地方都有时，环境变量优先。

## 用法

```bash
afdian whoami                    # 验证凭据是否可用
afdian plans                     # 列出已上架的方案
afdian plans 2                   # 第 2 页
afdian orders                    # 查订单
afdian checkout <plan_id>        # 生成收银台链接
afdian help
```

`checkout` 会输出一个 `https://afdian.com/order/create?...` 链接，把用户跳过去即可付款。

### 关联本站订单

`custom_order_id` 会原样带回，用它把爱发电订单和你的系统对上：

```bash
afdian checkout 0e65b484c2d411f192e852540025c377 ORD-2026-001
```

不带这个参数时会自动生成一个基于纳秒时间戳的单号。

## 输出

面向管道设计：成功走 stdout，失败走 stderr，退出码区分类型。

| 退出码 | 含义 |
| --- | --- |
| 0 | 成功 |
| 1 | 接口调用失败 |
| 2 | 凭据缺失或格式错误 |
| 64 | 命令行参数错误 |

```bash
afdian plans | grep 上架
```

## 设计取舍

**手写 MD5，不引加密库。** 算法固定，几十行就够，省一个依赖。有测试向量验证（RFC 1321 的四个标准值）。

**凭据只走环境变量或文件。** 命令行参数会留在 shell history 和 `ps` 输出里，是真实的泄漏渠道。

**紧凑 JSON 自己拼。** `serde_json` 默认会转义非 ASCII，而爱发电的签名要求中文原样参与计算。自己拼能保证行为确定。

## 开发

本仓库**不在本地编译**。改完推上去，CI 会跑：

- `cargo fmt --check`
- `cargo clippy -D warnings`
- `cargo test`（Linux / Windows / macOS 三平台）
- 四个目标平台的 release 产物

打 tag 会自动发 Release：

```bash
git tag v0.1.0 && git push origin v0.1.0
```

## 许可

[MIT](LICENSE)
