# Continuum 架构

## 边界

Continuum 是单机 Tauri 应用。React 只经类型化 IPC 调用 Rust；Rust 负责 SQLite、文件系统、只读 Git、Agent Adapter、上下文编译和进程启动。没有远程数据层，浏览器预览不伪造业务数据。

```text
React unified-project UI
  → appApi / Tauri commands
  → session_scanner → AgentAdapter → CodexAdapter
  → UnifiedProject / ConversationGraph / SQLite
  → ContextCompiler → ContextSnapshot → bootstrap Markdown
  → Fresh launcher → Codex CLI in explicit cwd
  → session watcher/detector → bind session ID → incremental sync
```

## Fresh Continuation 状态机

```text
compiling
  → writing_context
  → prepared
  → launching
  → waiting_detection
      ├─ one strict candidate → listening
      ├─ multiple candidates → needs_confirmation → listening
      └─ spawn error → launch_failed
```

`create_continuation(..., launch=false)` 完成快照与文件落盘；`launch_continuation` 单独启动进程，因此前端能显示真实阶段。支持 App Server 且 Profile 没有 CLI 专用参数时，Fresh 通过 `thread/start` 创建干净会话，先确认返回的 thread ID 未被其他 Continuation 或项目绑定使用，再调用 `turn/start` 注入短提示并绑定；`on-request` / `untrusted` 的命令、网络和文件修改审批由全局连接管理器 relay 到 Continuum UI。App Server reader 同步把 thread、turn、item 和 error 生命周期通知投影到规范化会话与统一时间线；高频 delta 不逐块写库，以 `item/completed` 为权威结果，本地 JSONL watcher 只读校验 canonical item ID 并补齐漏项。不能无损映射时回退到 `codex -C <cwd> <short-prompt>` 与严格文件检测。两条 Fresh 路径都不进入 `resume` 或 `fork`。原生 Resume/Fork 使用独立命令。

## 新会话识别

每次续接生成 `CONTINUATION_ID=cont_YYYYMMDD_xxxxxxxx`。检测器扫描 Codex Adapter 的真实会话目录，并同时要求：

1. 会话创建时间严格晚于进程启动时间（RFC 3339 比较）。
2. 规范化后的工作目录等于项目目录。
3. 第一条用户消息包含该 continuation marker。
4. 来源 Adapter/Agent 为 Codex。
5. session ID 尚未绑定任何统一项目。

单候选自动绑定；多候选交给用户确认；零候选继续轮询。绑定完成后每次 poll 调用增量同步，节点 ID 由 project、branch、source session 和 source message 组合，重复 poll 不重复插入。

## Context Compiler

`ContextCompressionProvider` 是可替换接口，第一版使用确定性的 `RuleBasedProvider`。编译结果包含系统上下文、编译文本、Token 估算、冲突和逐项解释。核心输入包括：

- 项目目标和长期约束；
- 关键决策、活跃文件、TODO、错误与失败尝试；
- 最近 N 轮消息和受限工具日志；
- 当前只读 Git 状态与可选 Diff；
- 项目已绑定的 Skills 和 MCP 摘要；
- 重复、完成、错误或陈旧节点的检索引用。

超预算时先把未固定的短期内容降级为 retrieval。在线 Provider 仅保留显式未启用的扩展点，不会静默调用 API。

## 数据模型

- `projects`：Agent 无关的统一项目状态。
- `conversation_branches` / `conversation_nodes`：分支图和来源可追溯节点。
- `source_sessions` / `source_messages`：第三方会话只读镜像。
- `project_bindings`：来源会话、Skill、MCP 与项目关系。
- `context_snapshots` / `context_items`：可重现且可解释的编译产物。
- `continuations`：文件、Hash、marker、PID、工作目录、时间、目标 session 和监听状态。
- `app_server_notifications` / `app_server_turns` / `app_server_items`：不含完整 payload 的紧凑生命周期账本和规范化 App Server 状态。

SQLite 使用 WAL、外键和 busy timeout。当前 Schema 为 v4；v2/v3 升级前创建可恢复备份，Windows 会先检查至少“数据库大小 + 128 MiB”的可用空间。所有关系都落盘，因此重启不依赖内存状态恢复。

## Windows 进程启动

启动器解析用户配置的 Codex 命令。在 Windows 上优先选择 npm `codex.cmd`，避免 `where.exe` 返回可见但拒绝直接启动的 Microsoft Store 内部 `codex.exe`；没有 `.cmd` 时再选择 `.exe` 或 PowerShell `.ps1` shim。上下文正文不放入命令行，只传入短启动提示和文件路径，从而规避命令行长度、换行与引号问题。

## 安全边界

Git Inspector 只执行固定只读命令并有超时。Continuum 不回写 Codex JSONL、不执行历史命令、不自动安装 Skill/MCP。bootstrap 文件写入项目内 `.continuum`，内容 Hash 与启动元数据写入数据库，便于审计。

旧 package/zip 模块保留为 legacy/export，不参与主路由和 Fresh Continuation。
