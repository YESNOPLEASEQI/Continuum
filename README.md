# Continuum

Continuum 是一个本地优先的跨 Agent 统一会话与上下文续接桌面客户端。第一版真实支持 Codex CLI：扫描本地 JSONL 会话，把多个来源会话绑定到同一统一项目和分支图，并通过 **Fresh Continuation** 将长历史编译成必要上下文后启动全新的干净 Codex 会话。

当前版本：`0.1.0-alpha.2`

## 第一核心流程

```text
扫描 Codex 本地会话
  → 选择来源会话并绑定统一项目/分支
  → Context Compiler 生成可解释快照
  → 写入 <project>/.continuum/continuations/<continuation-id>.md
  → 在项目工作目录启动全新 Codex CLI
  → 首条提示携带唯一 CONTINUATION_ID 并要求读取快照文件
  → 按启动时间 + 工作目录 + 唯一标识 + Agent 类型 + 未绑定状态识别新会话
  → 绑定新 session ID 并插入会话切换节点
  → 持续增量同步新 JSONL 消息到统一时间线
```

Fresh Continuation 不调用 `codex resume` 或 `codex fork`。三个操作在界面中独立呈现：

- Resume：继续原有长会话。
- Fork：从原生历史分叉，仍可能继承旧历史。
- Fresh Continuation：创建全新会话，只注入编译后的必要上下文；这是主功能。

## 已实现能力

- 扫描默认 `~/.codex/sessions` 和用户配置的真实会话目录；坏 JSONL 行只产生警告，不生成演示数据。
- 创建 Unified Project，并保留项目目标、长期约束、工作目录、分支和来源 Agent/session。
- 将多个来源会话按时间显示在统一时间线，支持节点分支、状态、重要度、工具调用和文件变化。
- 确定性 `RuleBasedProvider`：将上下文分为 permanent、phase、short-term、retrieval，并为每项记录 `keep | compress | retrieve | exclude` 与原因。
- 将当前只读 Git 状态、可选 Diff、失败尝试、近期消息，以及项目已绑定的 Skills/MCP 摘要纳入编译。
- 保存 Context Snapshot、上下文文件 SHA-256、启动时间、进程 ID、工作目录、唯一 marker、目标 session ID 和监听状态。
- 新会话自动识别使用组合校验；零候选继续等待，多候选进入人工确认，不以“最新文件”单独判断。
- 绑定后轮询来源 JSONL，只增量插入新消息；SQLite 关系在应用重启后保持。
- Claude Code 保留适配器框架；尚未确认安全启动能力时只导出上下文，不伪装成原生续接。

旧的 `.agentpack.zip` 代码仅作为 `legacy/export` 基础设施保留，不在主导航或核心用户流程中。

## 技术栈

- Tauri 2、Rust 2021、SQLite（`rusqlite` bundled）
- React 19、TypeScript strict、Vite、React Router、Zustand
- Vitest、Testing Library、Playwright

## Windows 开发环境

需要 Node.js 20+、Rust stable MSVC、Cargo、Microsoft C++ Build Tools（Desktop development with C++ 与 Windows SDK）、WebView2 Runtime、Git 和 Codex CLI。

```powershell
npm install
npm run tauri:dev
```

仅预览前端布局：

```powershell
npm run dev
```

浏览器模式没有 SQLite 或文件系统权限，真实扫描、编译、启动和监听必须在 Tauri 桌面运行时中执行。

## 验证

```powershell
npm run typecheck
npm test
npm run build
cd src-tauri
cargo test
```

Rust 集成测试会在临时目录创建 JSONL 与 SQLite 数据库，覆盖来源会话导入、上下文快照、Continuation marker 检测、新 session ID 绑定、增量消息同步和数据库重开后的持久化。另有一个默认忽略的真实验收测试；它需要本机已登录的 Codex CLI，并会永久创建一条本地 Codex 会话：

```powershell
cd src-tauri
cargo test --lib real_app_server_fresh_continuation_creates_binds_and_indexes_a_session -- --ignored --nocapture
```

## 数据与安全

- 主数据库为 Tauri 应用数据目录中的 `continuum.sqlite3`。
- 上下文文件位于源项目的 `.continuum/continuations/`，避免 Windows 命令行长度、换行和引号限制。
- Continuum 不写入第三方会话 JSONL，不自动执行历史命令，不自动修改或重置 Git。
- Context Compiler 默认不调用在线模型 API；Token 数为确定性估算，Context Health 是风险信号而非模型能力测量。
- Fresh 启动提示要求 Codex 先核对工作目录、Git 和实际文件；冲突时以实际工作区为准。

产品边界见 [产品定义](docs/product-definition.md)，实现结构见 [架构](docs/architecture.md)。
