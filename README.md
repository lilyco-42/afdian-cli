# afdian-cli

爱发电开放接口的命令行工具。签名、紧凑 JSON、凭据管理与格式校验都固化在二进制里。

> **本仓库禁止本地编译与运行。** 所有构建、测试与产物都在 GitHub Actions 上完成。
> 详见 [AGENTS.md](AGENTS.md)。

## 为什么需要它

对接爱发电时，下面这几件事每次都要重写一遍：

- 签名算法（顺序错了只返回 `sign validation failed`，看不出哪里错）
- 紧凑 JSON（`separators=(',',':')`，中文不能转义）
- 凭据怎么放（塞命令行参数会漏进 shell history）
- 方案 ID 的格式校验（32 位十六进制）

这个工具把上面四件事固化下来，一个二进制搞定。

## 安装

### cargo binstall（推荐）

```bash
cargo binstall afdian-cli
```

一条命令装好，不编译。

### 直接下载

从 [Releases](https://github.com/lilyco-42/afdian-cli/releases) 下载对应平台：

| 平台 | 文件 |
| --- | --- |
| Linux x86_64 | `afdian-x86_64-unknown-linux-musl` |
| Linux aarch64 | `afdian-aarch64-unknown-linux-musl` |
| Windows x86_64 | `afdian-x86_64-pc-windows-msvc.exe` |
| macOS Apple Silicon | `afdian-aarch64-apple-darwin` |

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

token 在 <https://afdian.com/dashboard/dev> 生成，页面同时会显示你的 `user_id`。

## 用法

```bash
afdian ping                        # 自检：验证凭据是否可用
afdian plan <plan_id>              # 查单个方案详情
afdian orders                      # 订单列表（每页 50 条）
afdian orders 2                    # 第 2 页
afdian orders --no <订单号>         # 按订单号查单条
afdian sponsors                    # 赞助者列表
afdian checkout <plan_id>          # 生成收银台链接
afdian help
```

### 关于「列出全部方案」

**爱发电没有提供这个接口。** 开放接口一共只有四个：

| 端点 | 参数 | 用途 |
| --- | --- | --- |
| `POST /api/open/ping` | 任意非空对象 | 连通性与凭据自检 |
| `POST /api/open/query-plan` | `{"plan_id": "..."}` | 查单个方案 |
| `POST /api/open/query-order` | `{"page": N}` 或 `{"out_trade_no": "..."}` | 查订单 |
| `POST /api/open/query-sponsor` | `{"page": N}` | 查赞助者 |

`query-plan` 必须给 `plan_id`，给空对象会返回 `params was not valid json string`，
给错 ID 返回 `方案不存在`。方案 ID 在 <https://afdian.com/dashboard/plans> 里找。

### 关联本站订单

`custom_order_id` 会原样带回，用它把爱发电订单和你的系统对上：

```bash
afdian checkout 63f94afacc9d11f19fbe5254001e7c00 LA42-2026-001
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
afdian orders | grep 已支付
```

## 设计取舍

**密码学来自 [`lilyco-crypto`](https://crates.io/crates/lilyco-crypto)，不自己写。**
MD5 用 RustCrypto 的 `md-5`。手写实现的风险不在算得对不对（标准向量能测出来），
而在侧信道——内存访问模式、分支预测、编译器把清零逻辑优化掉。这些测试发现不了。

**凭据只走环境变量或文件。** 命令行参数会留在 shell history 和 `ps` 输出里，是真实的泄漏渠道。

**紧凑 JSON 自己拼。** `serde_json` 默认会转义非 ASCII，而爱发电的签名要求中文原样参与计算。
自己拼能保证行为确定，键名排序也稳定。

**命令骨架抄自 `tiny_lilyco`，但只内联了两个类型。** crates.io 不接受 git 依赖，
而 `tiny_lilyco` 没发布到 crates.io——引了就没法 `cargo binstall`。用到的只有
`CommandSchema` 和 `Progress`，各几行，抄进 `src/schema.rs` 更划算。

## 签名算法

```
sign = md5(token + "params" + params_json + "ts" + ts + "user_id" + user_id)
```

注意 `token` 是**直接顶在最前面**，前面没有 `"token"` 这个字面量。
这个顺序是 2026-10 用真实凭据验证的：服务端在 `data.debug.kv_string`
里回显了它自己拼的原文，`lilyco-crypto` 里有对应的回归测试锁死。

## 开发

本仓库**不在本地编译**。改完推上去，CI 会跑：

- `cargo fmt --check`
- `cargo clippy -D warnings`
- `cargo test`（Linux / Windows / macOS 三平台）
- 四个目标平台的 release 产物

打 tag 会自动发 Release 并发布到 crates.io：

```bash
git tag v0.1.3 && git push origin v0.1.3
```

## 许可

[MIT](LICENSE)
