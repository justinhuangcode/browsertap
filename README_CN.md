# BrowserTap

[English](./README.md) | **中文**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![TypeScript](https://img.shields.io/badge/typescript-5.7%2B-blue.svg?style=flat-square&logo=typescript&logoColor=white)](runtime/browser/)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)](https://github.com/justinhuangcode/browsertap)
[![GitHub Stars](https://img.shields.io/github/stars/justinhuangcode/browsertap?style=flat-square&logo=github)](https://github.com/justinhuangcode/browsertap/stargazers)
[![Last Commit](https://img.shields.io/github/last-commit/justinhuangcode/browsertap?style=flat-square)](https://github.com/justinhuangcode/browsertap/commits/main)
[![Issues](https://img.shields.io/github/issues/justinhuangcode/browsertap?style=flat-square)](https://github.com/justinhuangcode/browsertap/issues)

实时浏览器控制命令行工具，支持活跃标签捕获、DOM 交互与 Agent 自动化。 🌐

接入你正在运行的浏览器，闭合 Agent 循环。browsertap 让 AI Agent 和 CLI 工具控制一个**已经打开、已经登录**的浏览器会话 -- 截图、JS 执行、冒烟测试、控制台捕获等 -- 无需启动无头浏览器实例或重新登录。

## 为什么选择 BrowserTap？

与 Web 应用交互的 AI Agent 需要**看到并控制真实环境**。它们需要执行 JS、截图、检查控制台错误、点击按钮 -- 所有这些都在一个已经登录的浏览器中完成，拥有真实的 Cookie、真实的会话和真实的状态。

现有工具无法满足这一工作流：

| | browsertap | Playwright | Puppeteer |
|---|---|---|---|
| 接入已打开的浏览器标签页 | **是** | 否（新实例） | 否（新实例） |
| 保留认证状态 | **是** | 否（需重新登录） | 否（需重新登录） |
| 运行时依赖 | **无**（单一二进制文件） | Node.js | Node.js |
| 二进制体积 | **~5 MB** | ~100 MB+ | ~100 MB+ |
| 启动时间 | **< 10 ms** | > 500 ms | > 500 ms |
| 内置冒烟测试 | **是**（并行） | 否 | 否 |
| 会话代号 | **是** | 否 | 否 |
| 控制台/网络缓冲 | **是** | 仅通过代码 | 仅通过代码 |
| 自签名 TLS | **内置**（rcgen + rustls） | 不适用 | 不适用 |
| 为 AI Agent 设计 | **是** | 否（测试框架） | 否（库） |

**典型的 AI Agent 与 browsertap 工作流：**

```
开发者在浏览器中打开 Web 应用（已登录）
        |
        v
@browsertap/runtime 将标签页连接到守护进程
        |
        v
AI Agent 执行: browsertap run-js iron-falcon "document.title"
        |
        v
AI Agent 执行: browsertap screenshot iron-falcon -o page.jpg
        |
        v
AI Agent 检查截图 / 查询 DOM / 检查控制台
        |
        v
AI Agent 执行: browsertap smoke iron-falcon --preset main
        |
        v
无需无头浏览器。无需重新登录。无需丢失状态。
```

## 功能特性

- **接入实时会话** -- 控制已打开、已认证的浏览器标签页
- **守护进程架构** -- `browsertapd` 作为持久化 HTTPS + WebSocket 中枢运行；CLI 命令通过 REST API 与之通信
- **会话代号** -- 使用友好的名称如 `iron-falcon` 或 `calm-otter`，而非 UUID
- **JavaScript 执行** -- 通过 CLI 在浏览器上下文中执行任意 JS
- **截图捕获** -- 全页面或通过 CSS 选择器捕获特定元素
- **控制台捕获** -- 查看浏览器控制台输出，支持级别过滤；缓冲区在 CLI 重连后仍然保留
- **网络捕获** -- 检查运行时缓冲的 HTTP 请求/响应
- **冒烟测试** -- 自动路由扫描，支持预设、错误检测和进度跟踪
- **选择器发现** -- 查找页面上的交互元素（按钮、链接、输入框）
- **HMAC-SHA256 令牌** -- 短期会话令牌（5 分钟）和 CLI 令牌（1 小时）
- **自签名 TLS** -- 通过 rcgen + rustls 自动生成证书，零外部工具依赖
- **自动重连** -- 浏览器运行时在断开连接后以指数退避策略重连
- **配置文件向上查找** -- 将 `browsertap.toml` 放在项目根目录；CLI 自动向上查找
- **JSON 输出** -- 机器可读的输出格式，便于 Agent 集成
- **跨平台** -- macOS、Linux 和 Windows

## 安装

### 预构建二进制文件（即将推出）

所有平台的预构建二进制文件将在 [GitHub Releases](https://github.com/justinhuangcode/browsertap/releases) 上提供。

| 平台 | 二进制文件 |
|---|---|
| Linux x86_64 | `browsertap-v*-linux-x86_64.tar.gz` |
| Linux ARM64 | `browsertap-v*-linux-arm64.tar.gz` |
| macOS Intel | `browsertap-v*-macos-x86_64.tar.gz` |
| macOS Apple Silicon | `browsertap-v*-macos-arm64.tar.gz` |
| Windows x86_64 | `browsertap-v*-windows-x86_64.zip` |

### 通过 Cargo

```bash
# 安装两个二进制文件
cargo install --path crates/cli
cargo install --path crates/daemon
```

### 浏览器运行时 SDK

```bash
npm install @browsertap/runtime
```

### 从源码构建

```bash
git clone https://github.com/justinhuangcode/browsertap.git
cd browsertap
cargo build --release
# 二进制文件位于: target/release/browsertap, target/release/browsertapd
```

**环境要求：** Rust 1.75+ 以及用于控制页面的 Chromium 内核浏览器。

## 快速开始

### 1. 启动守护进程

```bash
browsertapd
# => browsertapd listening on https://127.0.0.1:4455
```

### 2. 将浏览器运行时集成到你的 Web 应用

```typescript
import { createBrowserTapClient, createSessionStorageAdapter } from '@browsertap/runtime';

const client = createBrowserTapClient({
  storage: createSessionStorageAdapter(),
  onStatus: (snap) => console.log('browsertap:', snap.status, snap.codename),
  autoReconnectHandshake: () =>
    fetch('/api/browsertap/handshake', { method: 'POST' }).then(r => r.json()),
});

const handshake = await fetch('/api/browsertap/handshake', { method: 'POST' }).then(r => r.json());
await client.startSession(handshake);
// => "connected as iron-falcon"
```

### 3. 通过 CLI 控制

```bash
browsertap sessions
# CODENAME             URL                                      STATE      HEARTBEAT
# iron-falcon          http://localhost:3000/dashboard           open       2s ago

browsertap run-js iron-falcon "document.title"
# "Dashboard - MyApp"

browsertap screenshot iron-falcon --selector "#analytics" -o card.jpg
# Screenshot saved to card.jpg (45832 bytes)
```

## 命令

| 命令 | 描述 |
|---|---|
| `daemon` | 启动 browsertap 守护进程（委托给 `browsertapd`） |
| `sessions` | 列出活跃的浏览器会话，包含代号和心跳状态 |
| `run-js <session> <code>` | 在浏览器会话中执行 JavaScript |
| `screenshot <session>` | 捕获页面或元素截图 |
| `click <session> <selector>` | 通过 CSS 选择器点击元素 |
| `navigate <session> <url>` | 将会话导航到指定 URL |
| `smoke <session>` | 在配置的路由上运行冒烟测试 |
| `console <session>` | 查看会话的控制台日志 |
| `selectors <session>` | 发现页面上的交互选择器 |

## 命令参数

### 全局参数

| 参数 | 默认值 | 描述 |
|---|---|---|
| `--daemon-url <url>` | `https://127.0.0.1:4455` | 守护进程 URL（也可通过 `BROWSERTAP_DAEMON_URL` 设置） |

### `screenshot` 参数

| 参数 | 默认值 | 描述 |
|---|---|---|
| `-s, --selector <sel>` | *（全页面）* | 要捕获的元素的 CSS 选择器 |
| `-o, --output <path>` | `screenshot.jpg` | 输出文件路径 |
| `--quality <f32>` | `0.85` | JPEG 质量（0.0 - 1.0） |

### `smoke` 参数

| 参数 | 默认值 | 描述 |
|---|---|---|
| `--preset <name>` | `defaults` | `browsertap.toml` 中的路由预设名称 |
| `--routes <list>` | *（无）* | 逗号分隔的路由列表 |
| `--parallel <n>` | `1` | 并行工作线程数 |

### `console` 参数

| 参数 | 默认值 | 描述 |
|---|---|---|
| `-t, --tail <n>` | `50` | 显示的最近事件数量 |
| `--level <level>` | *（全部）* | 按级别过滤：log, info, warn, error |

## 工作原理

1. **`browsertapd`** 在 `127.0.0.1:4455` 上启动 HTTPS + WebSocket 服务器。首次运行时自动生成自签名 TLS 证书，并存储在 `~/.browsertap/certs/`。

2. **你的 Web 应用** 嵌入 `@browsertap/runtime`。激活时，运行时调用后端的握手端点，该端点使用共享密钥签发 HMAC-SHA256 会话令牌。

3. **浏览器运行时** 向守护进程打开 WebSocket 连接，发送带有签名令牌的 `register` 消息，并接收一个友好的代号（如 `iron-falcon`）。然后它会 patch `console.*` 以捕获日志，并每 5 秒发送一次心跳。

4. **CLI 命令**（`browsertap run-js iron-falcon "..."`）向守护进程的 REST API 发送 HTTPS 请求。守护进程通过 WebSocket 将命令转发给浏览器，等待结果后返回给 CLI。

5. **控制台和网络事件** 在守护进程中缓冲（每个会话 500 条控制台事件、200 条网络事件）。CLI 可以回溯查询这些缓冲区，即使是在 CLI 连接之前发生的事件。

## 架构

```
                            WebSocket (wss://)
+------------------+                                +------------------+
|  你的 Web 应用    |  ------>  +--------------+     |  CLI / AI Agent  |
|  （已登录）       |  <------  |  browsertapd |     |                  |
|                  |           |              |     |  $ browsertap    |
| @browsertap/     |  register |  Session     | <-- |    run-js        |
|   runtime        |  heartbeat|  Registry    | --> |    screenshot    |
|                  |  console  |  Command     |     |    smoke         |
|                  |  result   |  Router      |     |    console       |
+------------------+           |  TLS (rustls)|     +------------------+
                               +--------------+
                                 HTTPS REST API
```

## 配置

在项目根目录创建 `browsertap.toml`。CLI 会自动向上查找该文件。

```toml
app_label = "MyApp"
app_url = "http://localhost:3000"
daemon_url = "https://127.0.0.1:4455"

[daemon]
host = "127.0.0.1"
port = 4455

[smoke]
defaults = ["dashboard", "settings", "profile"]

[smoke.presets]
main = ["dashboard", "settings", "profile", "billing"]
quick = ["dashboard"]

[smoke.redirects]
"/" = "/dashboard"
```

**配置优先级：** CLI 参数 > 环境变量 > `browsertap.toml` > 默认值

### 环境变量

| 变量 | 描述 |
|---|---|
| `BROWSERTAP_DAEMON_URL` | 守护进程 URL |
| `BROWSERTAP_HOST` | 守护进程监听地址 |
| `BROWSERTAP_PORT` | 守护进程监听端口 |
| `BROWSERTAP_SECRET` | 共享密钥（十六进制字符串） |

## 后端握手端点

你的 Web 应用后端需要一个端点来签发会话令牌：

```typescript
// POST /api/browsertap/handshake
import { readFileSync } from 'fs';
import { createHmac, randomUUID } from 'crypto';

export async function POST() {
  const secret = process.env.BROWSERTAP_SECRET
    ?? readFileSync(`${process.env.HOME}/.browsertap/secret.key`, 'utf8').trim();

  const sessionId = randomUUID();
  const payload = {
    token_id: randomUUID(),
    scope: 'session',
    subject: 'browsertap-web',
    session_id: sessionId,
    issued_at: new Date().toISOString(),
    expires_at: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  };

  const encoded = Buffer.from(JSON.stringify(payload)).toString('base64url');
  const sig = createHmac('sha256', Buffer.from(secret, 'hex'))
    .update(encoded).digest('base64url');

  return Response.json({
    sessionId,
    sessionToken: `${encoded}.${sig}`,
    socketUrl: 'wss://127.0.0.1:4455/bridge',
    expiresAt: Math.floor(Date.now() / 1000) + 300,
  });
}
```

## 安全与威胁模型

browsertap 设计用于开发机器上的**单用户、仅本地**使用。

| 层级 | 控制措施 | 详情 |
|---|---|---|
| **HTTPS 服务器** | 仅本地访问 | 绑定到 `127.0.0.1`；不暴露到网络 |
| **TLS** | 自动生成证书 | 通过 rcgen + rustls 自签名，存储于 `~/.browsertap/certs/` |
| **会话令牌** | HMAC-SHA256，短期有效 | 浏览器令牌 5 分钟过期；CLI 令牌 1 小时过期 |
| **令牌验证** | 恒定时间比较 | 使用 `hmac` crate 的时间安全比较 |
| **密钥存储** | 仅所有者权限 | `~/.browsertap/secret.key` 以 `0600` 模式创建（Unix） |
| **控制台缓冲** | 有界限制 | 每个会话最多 500 条事件，防止内存耗尽 |

### 不推荐用于

- **多用户 / 共享机器** -- 拥有 root 权限的其他本地用户可以读取会话令牌
- **生产环境** -- browsertap 是开发/测试工具；没有速率限制或审计日志
- **不受信任的网络** -- 自签名证书默认不进行验证

## 项目结构

```
browsertap/
├── Cargo.toml                    # 工作区根配置
├── browsertap.toml               # 项目配置示例
├── crates/
│   ├── shared/                   # 共享库（令牌、协议、类型）
│   │   └── src/
│   │       ├── lib.rs            # 模块导出
│   │       ├── token.rs          # HMAC-SHA256 令牌签发/验证
│   │       ├── protocol.rs       # WebSocket + REST 协议类型
│   │       ├── session.rs        # 会话状态、配置类型
│   │       └── codename.rs       # 友好代号生成
│   ├── daemon/                   # 守护进程二进制文件 (browsertapd)
│   │   └── src/
│   │       ├── main.rs           # Axum HTTPS 服务器 + REST 路由
│   │       ├── state.rs          # 会话注册表、命令路由
│   │       ├── websocket.rs      # WebSocket 处理器（注册/心跳/命令）
│   │       └── tls.rs            # 自签名证书生成 (rcgen)
│   └── cli/                      # CLI 二进制文件 (browsertap)
│       └── src/
│           ├── main.rs           # Clap 命令定义
│           ├── client.rs         # 守护进程 REST API 的 HTTP 客户端
│           └── config.rs         # browsertap.toml 加载器（向上查找）
└── runtime/
    └── browser/                  # 浏览器运行时 SDK (TypeScript)
        ├── package.json          # @browsertap/runtime
        ├── tsconfig.json
        └── src/
            ├── index.ts          # 公共 API 导出
            ├── client.ts         # WebSocket 生命周期、命令执行器、控制台 patch
            ├── types.ts          # TypeScript 类型定义
            └── storage.ts        # 会话持久化适配器
```

## 路线图

- [ ] 从 Chrome 主配置文件同步 Cookie
- [ ] 内置 OAuth 自动化（GitHub、Google、Twitter）
- [ ] 并行冒烟测试
- [ ] 视觉回归测试（截图差异比对）
- [ ] 网络请求拦截
- [ ] 状态快照（保存/恢复 Cookie + localStorage）
- [ ] 实时事件流（SSE）
- [ ] 操作系统钥匙串集成
- [ ] WebDriver BiDi 支持（Firefox）
- [ ] WASM 插件系统
- [ ] CI/CD 流水线（GitHub Actions）
- [ ] 预构建二进制发布
- [ ] Homebrew tap

## 贡献

欢迎贡献！请在提交 PR 之前先开一个 Issue 讨论你的想法。

## 更新日志

查看 [Releases](https://github.com/justinhuangcode/browsertap/releases) 了解版本历史。

## 许可证

[MIT](LICENSE)
