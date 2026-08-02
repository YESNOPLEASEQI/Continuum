# 在 ChatGPT Codex 客户端能力之上开发

更新日期：2026-08-01  
本机验证版本：`codex-cli 0.146.0` / `Codex Desktop/0.146.0`

## 结论

Continuum 不应修改或注入 ChatGPT/Codex 官方桌面应用本体。官方为自建富客户端提供的深度集成边界是 **Codex App Server**：它提供认证、会话历史、审批和流式 Agent 事件，并且正是 Codex 富客户端使用的接口。Continuum 应作为独立 Tauri 客户端，通过本机 `codex app-server` 与 Codex 通信。

官方资料：

- [Codex App Server](https://learn.chatgpt.com/docs/app-server.md)
- [Codex CLI reference](https://developers.openai.com/codex/cli/reference)
- [Codex open source repository](https://github.com/openai/codex/tree/main/codex-rs/app-server)

## 已在本机验证的事实

1. `codex --version` 返回 `codex-cli 0.146.0`。
2. `codex app-server --help` 存在，并支持 `stdio://`、WebSocket 和 Unix socket；其中 WebSocket 仍标记为实验性且不受支持。
3. `codex app-server generate-json-schema --out <DIR>` 成功生成当前安装版本的 v1/v2 JSON Schema。
4. 通过 `stdio://` 发送 `initialize` 后，本机返回：

   ```text
   Codex Desktop/0.146.0 ... (continuum_probe; 0.1.0-alpha.1)
   ```

5. 当前 v2 Schema 确认 `thread/start` 支持 `cwd`、`model`、`approvalPolicy`、`sandbox` 和 `serviceName`；`turn/start` 支持文本输入并要求明确的 `threadId`。

这些是对当前安装环境的实测，不代表未来版本永远保持完全相同的字段。构建和发布时应使用目标 Codex 版本重新生成 Schema，并执行协议探针。

## 对 Continuum 的推荐架构

```mermaid
flowchart LR
  UI["Continuum React UI"] --> Tauri["Tauri command layer"]
  Tauri --> Compiler["Context Compiler"]
  Compiler --> File["临时 Context Markdown"]
  Tauri --> AS["Codex App Server adapter · stdio JSON-RPC"]
  AS --> Codex["Fresh Codex thread + turn"]
  AS --> Events["Lifecycle notification normalizer"]
  Events --> DB["Continuum SQLite + unified timeline"]
  Codex --> Store["Codex local session store"]
  Store --> Watcher["JSONL incremental verifier"]
  Watcher -. "canonical ID 去重 / 漏项补齐" .-> DB
  AS -. "旧版或 CLI 专用参数" .-> Fallback["CLI launch + strict marker detection"]
  Fallback --> Store
```

主路径使用 App Server，原因是 `thread/start` 直接返回新 thread ID，不需要猜测“最新会话文件”，生命周期通知也能立即形成规范化持久视图。本地 JSONL 扫描仍然有价值：它是重启恢复、历史导入、兼容旧版本、canonical ID 去重校验和异常退出补漏的只读后备。

## Fresh Continuation 的正确协议映射

1. Continuum 在 SQLite 中先创建 Continuation 记录并编译 Context Snapshot。
2. 把长上下文写入项目内临时 Markdown 文件，记录 SHA-256。
3. 启动 `codex app-server --listen stdio://`。
4. 发送一次 `initialize`，`clientInfo.name` 使用 `continuum`；随后发送 `initialized`。
5. 调用 `thread/start`，传入显式 `cwd`、Profile 的 model/approval/sandbox，并保持 `ephemeral=false`。
6. 直接读取响应中的 `thread.id`，立即绑定到统一项目和原分支。
7. 调用 `turn/start`，首条文本只包含唯一 `CONTINUATION_ID`、上下文文件路径和核对工作区的指令。
8. 继续读取通知并直接持久化 thread/turn/item/error 生命周期；同时监听本地会话存储，以 JSONL 增量索引复用 canonical item ID、校验结果并补齐漏项。

Fresh 不应调用：

- `thread/resume`：它继续旧 thread。
- `thread/fork`：它复制旧历史形成分叉。
- `thread/compact/start`：它压缩的是现有 thread，而不是创建只含 Continuum 编译上下文的干净 thread。
- `thread/inject_items`：这是更底层的历史注入接口，当前需求用“上下文文件 + 一条短用户提示”更容易审计、跨版本也更稳健。

## 稳定性与安全边界

- 使用本地 `stdio://`；不要为本机桌面客户端暴露无认证的非回环 WebSocket。
- 在发送任何请求前完成 initialize/initialized 握手。
- 为请求 ID、响应超时、JSON 解析失败和进程退出分别记录错误码。
- App Server 能力检测成功后才启用主路径；旧版 Codex 回退到现有 CLI + marker + cwd + createdAt 多条件识别。
- Profile 含无法映射到协议字段的 CLI 启动参数时，必须走 CLI 后备，不能静默丢弃参数。
- 无 CLI 专用启动参数时，`never`、`on-request` 和 `untrusted` Profile 均可走 App Server。命令执行、网络上下文、文件修改、权限申请、MCP elicitation 和工具用户输入进入 Continuum 全局 UI；其他未知请求显式失败，不能在后台无响应地等待。
- 不把长上下文放到命令行；仍通过临时 Markdown 文件传递。
- 通知账本只保存 hash、method、thread/turn/item ID 和时间，不保存完整 payload；token、reasoning、command output、patch 等高频 delta 合并到权威 `item/completed`，避免 SQLite 写放大。
- `clientInfo.name` 会进入合规日志。面向企业正式发布前，应按官方说明联系 OpenAI 登记已知客户端标识。
- 默认主链路只使用稳定方法；为支持 `item/tool/requestUserInput` 和 OpenAI MCP 表单，Continuum 在 initialize 中显式声明 `experimentalApi` 与 `mcpServerOpenaiFormElicitation`，协议升级时必须重新核对 Schema。

## 在 Codex 客户端中开发 Continuum 的工作方式

“在 Codex 客户端中开发”适合分成三层：

1. 当前任务提示：只放本轮目标、边界和验收。
2. 仓库 `AGENTS.md` 与 `.codex/config.toml`：放长期工程约定、验证命令、沙箱/MCP/Hook 配置。
3. 可复用 Skill/Plugin/MCP：放可安装工作流或外部数据与动作。

不要通过修改 Codex 安装目录或依赖桌面 UI 控件位置实现产品能力。Continuum 的稳定接口应是 App Server、CLI 能力检测和公开的本地会话持久化格式；桌面 UI 自动化只能用于端到端冒烟测试。

## 已落实到代码的改动

- 新增 `codex_app_server` Rust 适配器，使用 stdio JSON-RPC 完成握手、`thread/start` 和 `turn/start`。
- Fresh Continuation 在能力允许且 Profile 可无损映射时优先使用 App Server；返回的 thread ID 必须先确认未被其他 Continuation 或项目绑定使用，才能执行 `turn/start` 并绑定。全局连接管理器保留 stdin 以响应后续审批。
- 命令执行、网络访问和文件修改审批请求按原 JSON-RPC `id` relay 到全局 UI，支持允许本次、本会话允许、拒绝和拒绝并停止；请求队列随子进程断开清理。
- `request_permissions`、MCP form/openai-form/URL elicitation 与工具用户输入使用同一全局请求队列，但按协议分别返回权限子集、action/content 和按问题 ID 组织的 answers；重复 RPC ID 去重，`serverRequest/resolved` 会清理已自动解决的请求。
- App Server reader 直接把 thread/turn/item/error 生命周期投影到规范化会话、工具调用、文件变化和 Unified Timeline；Schema v4 保存紧凑通知账本，JSONL 全量与增量索引作为只读 verifier/fallback，并复用 App Server canonical item ID。
- 原 CLI 启动与严格本地会话识别保留为后备。
- Diagnostics 增加 App Server 能力显示与无副作用握手探针。
- Codex Profile 显示 App Server 能力；CLI 专用参数会阻止无损映射并使用后备路径。
- 2026-08-01 的隔离只读真实验收已覆盖新 thread ID、真实 JSONL、assistant 增量消息与数据库重开持久化；证据见 `fresh-continuation-acceptance-2026-08-01.md`。
- Windows fake App Server 子进程测试覆盖响应乱序、JSON-RPC 错误、响应超时、进程提前退出和重复 thread ID；重复 ID 会在 `turn/start` 前拒绝，相关 `thread/started` 通知也不会覆写旧会话记录。

## 后续实现优先级

1. 在应用退出时管理 App Server 子进程，在重启后恢复为“已绑定、等待本地持久化同步”。
2. 发布构建中生成并保存目标 Codex 版本的协议 Schema Hash，启动时对比本机版本。
