# Continuum 项目交接文档

更新时间：2026-08-02
当前版本：`0.1.0-alpha.2`
公开仓库：<https://github.com/YESNOPLEASEQI/Continuum>
默认分支：`main`

## 1. 文档用途

本文档用于让新的开发者或新的 Codex 对话在不依赖旧聊天记录的情况下接管 Continuum。根目录的 `AGENTS.md` 是每次对话必须读取和维护的权威项目总账；本文档提供更完整的产品、架构、运行、测试和交接说明。

状态来源优先级：当前代码与测试、`AGENTS.md`、本文档、`docs/remaining-work.md`、其他带日期的历史审计和日志。较早文档不得覆盖更新后的代码事实。

## 2. 产品定义

Continuum 是本地优先的 Windows 桌面客户端，用于把本机 Codex 会话组织为统一项目、分支和时间线，并把冗长的旧会话编译为可解释的必要上下文，从而启动干净的新 Codex 会话。

产品的核心不是复制聊天文本或导出任务包，而是自动完成以下闭环：

1. 扫描 Codex 本地会话；
2. 把来源会话绑定到 Unified Project 和 Conversation Branch；
3. 分析来源会话、项目约束、Git 与实际工作区；
4. 生成 Context Snapshot 和压缩 Markdown；
5. 在原项目工作目录启动全新的 Codex 会话；
6. 注入唯一 `CONTINUATION_ID` 和上下文读取指令；
7. 获取或严格识别新 session ID；
8. 把新会话绑定回原项目和原分支；
9. 持续增量导入新消息；
10. 在统一时间线中显示会话切换节点。

### 三种操作必须保持独立

- **Resume**：恢复原会话，继续原有长历史。
- **Fork**：从原历史分叉，仍可能继承旧历史。
- **Fresh Continuation**：创建干净的新会话，只注入 Continuum 编译后的必要上下文。这是默认主功能。

Fresh 不得通过 `codex resume`、`codex fork`、`thread/resume` 或 `thread/fork` 实现。

### 非目标和安全边界

- 不修改 Codex 官方客户端安装文件，也不依赖其 UI 控件位置。
- 不回写、删除或重排第三方 Agent 原始会话 JSONL。
- 不自动执行历史聊天中的命令或破坏性 Git 操作。
- 不把长上下文放入 Windows 命令行。
- 不把密钥、环境变量值、完整本地会话或数据库提交到仓库。
- 浏览器预览不伪造桌面业务数据；真实扫描、启动和监听只在 Tauri 中工作。
- 第一阶段只宣称真实支持 Codex；其他 Adapter 目前只是扩展边界。

## 3. 核心架构

```text
React 19 / TypeScript UI
  -> typed bridge / Tauri IPC
  -> Rust command layer
  -> SQLite persistence
  -> Codex App Server notification index + incremental JSONL verifier
  -> Unified Project / branch / timeline
  -> Context Compiler V2 + Context Snapshot
  -> project-local .continuum/continuations/*.md
  -> Codex App Server (preferred) or CLI fallback
  -> new Codex session ID
  -> binding + watcher + unified timeline
```

技术栈：Tauri 2、Rust 2021、bundled SQLite、React 19、TypeScript strict、Vite、React Router、Zustand、GSAP、`@gsap/react`、Vitest、Testing Library 和 Playwright。当前目标平台是 Windows 11 x64。

## 4. Fresh Continuation 实现

### App Server 主路径

当本机 Codex 支持 App Server，且 Profile 没有只能映射到 CLI 的参数时：

1. 启动 `codex app-server --listen stdio://`；
2. 完成 `initialize` / `initialized`；
3. 调用 `thread/start`，传入显式 `cwd`、model、approval、sandbox；
4. 直接读取返回的 `thread.id`，并在注入上下文前确认它未被其他 Continuation 或项目绑定使用；
5. 调用 `turn/start`，发送短启动提示；
6. 绑定新 thread；
7. 直接持久化 thread/turn/item 生命周期通知并投影到统一时间线，同时让 JSONL watcher 使用 canonical item ID 进行去重校验和漏项补齐。

`never`、`on-request` 和 `untrusted` 均可走该路径。App Server 子进程的 stdin 由全局连接管理器持有；命令执行、网络上下文、文件修改、`request_permissions`、MCP elicitation 和工具用户输入请求进入内存队列，由全局 UI 展示并按各自协议写回原 JSON-RPC `id`。初始化显式协商 experimental API 与 OpenAI MCP 表单能力；重复请求 ID 去重，`serverRequest/resolved` 或进程断开时清理。请求队列不写入 SQLite，秘密回答和表单内容不会持久化；未知请求返回显式 `-32601`，不会静默悬挂。

通知持久化使用 Schema v4 的紧凑 lifecycle 账本和规范化 turn/item 表，不保存完整通知 payload。`item/completed` 是消息、工具输出和文件变化的权威值；token、reasoning、command output、patch 等高频 delta 不逐块写入 SQLite，避免恢复旧数据库的写放大。工具字段最多保存 256K 字符，file change 只保存 path/kind，不保存 diff。通知持久化失败会写入 Diagnostics，但不阻塞 App Server 协议响应。

App Server 握手可靠性由隔离的 Windows fake 子进程覆盖：乱序响应、JSON-RPC 错误、响应超时、进程提前退出和重复 thread ID。已使用的 thread ID 在 `turn/start` 前被拒绝，不能作为 partial thread 绑定；只有新 thread 已创建但首次 turn 失败时才记录 partial 状态，防止重复 Fresh 污染旧会话。

### CLI 后备路径

App Server 不可用或 Profile 含协议无法无损表达的参数时：

1. 使用显式工作目录启动 Codex CLI；
2. 命令行只传入短提示和上下文路径；
3. 监听 Codex 会话存储；
4. 使用创建时间、规范化 cwd、唯一 marker、Agent 类型和未绑定状态综合识别；
5. 一个严格候选自动绑定，多个候选要求手工确认，零候选继续等待。

不能只根据“最新文件”识别新会话。

### 状态机

```text
compiling -> writing_context -> prepared -> launching -> waiting_detection
  -> listening
  -> needs_confirmation -> listening
  -> launch_failed
```

恢复操作包括重试、重新扫描、手工绑定、取消和清理临时上下文。状态迁移必须持久、幂等，不能因重复点击创建多个空会话。

## 5. Codex Profiles

Profile 是 Codex 会话的启动配置预设，不是聊天内容。它可记录模型、推理强度、审批策略、沙箱、Codex 配置名、启动通道和额外参数。

- Fresh：必须可选择，默认继承当前分支或项目设置；
- Fork：允许选择，因为它创建新会话；
- Resume：默认保持原会话配置，覆盖项只放高级设置；
- 新建空白会话：应选择 Profile；
- 只新建逻辑分支：记录默认 Profile，在真正启动时确认。

继承优先级：

```text
全局默认 -> 项目默认 -> 分支默认 -> 本次启动临时选择
```

本次选择不能静默修改上层默认值。每次启动应保存不可变配置快照，避免同名 Profile 后续修改破坏审计和复现。

## 6. 数据与持久化

主数据存储在 Tauri 应用数据目录的 `continuum.sqlite3`。SQLite 使用 WAL、外键和 busy timeout。当前数据库 Schema 为 v4。

v2/v3 升级到 v4 前会 checkpoint 并创建可恢复备份；Windows 会先确认至少有“数据库大小 + 128 MiB”可用空间，复制失败会删除半成品并中止迁移。该迁移只新增 App Server 规范化表，不执行 `VACUUM`，因此旧 2.22 GB 文件的压缩仍是独立 P0。

主要实体包括：

- `projects`；
- `conversation_branches`、`conversation_nodes`；
- `source_sessions`、`source_messages`；
- `session_messages`、`session_tool_calls`、`file_changes`；
- `project_bindings`；
- `context_snapshots`、`context_items`；
- `continuations`；
- `app_server_notifications`、`app_server_turns`、`app_server_items`；
- Codex Profiles、备份记录、增量游标及配置表。

项目内临时上下文位于：

```text
<project>/.continuum/continuations/<continuation-id>.md
```

`.continuum/`、SQLite、构建目录和测试输出必须保持在 `.gitignore` 中。

## 7. 当前已实现成果

### 会话、项目和时间线

- 扫描默认和自定义 Codex JSON/JSONL 目录；
- 容忍损坏单行并记录警告；
- 持久字节游标，只解析追加内容；
- Source Sessions 按 session ID 只读关联 Codex `~/.codex/state_5.sqlite` 的 `threads.name/title/source`，显示与 Codex Desktop 侧边栏一致的正式标题并区分 Desktop/CLI；状态库不可用时才跳过协议注入内容并以首条真实用户请求兜底。列表已收敛为标题、来源、项目、时间、绑定状态、Fresh 主操作和溢出菜单；
- 创建、重命名、归档、恢复、迁移和删除 Unified Project 记录；
- 来源会话绑定、解绑、重新绑定和项目推荐；
- 统一时间线、来源追踪、搜索、过滤、分页、备注、复制和固定；
- 分支创建、切换、重命名、归档、恢复和安全删除。

### 上下文与续接

- 确定性 Context Compiler V2；
- permanent、phase、short-term、retrieval 四层内容；
- `keep | compress | retrieve | exclude` 决策和理由；
- 项目、会话、Git、Skills/MCP 等实际输入；
- Context Snapshot、Hash、Diff 和部分人工 override；
- Resume、Fork、Fresh 三种入口分离；
- App Server 与 CLI 两条 Fresh 路径；
- App Server 命令、网络、文件修改、权限申请、MCP elicitation 和工具用户输入 relay 与全局响应 UI；
- App Server thread/turn/item/error 通知直接持久化到规范化会话和 Unified Timeline，JSONL 作为只读 verifier/fallback；
- fake App Server 子进程覆盖乱序、错误、超时、退出和重复 thread ID，重复会话会在上下文注入前拒绝；
- 唯一 marker、严格候选识别、手工绑定；
- 新 session ID 持久绑定、增量监听和重启恢复；
- 真实 Codex Fresh 端到端验收通过。

### 配置、诊断和发布

- Codex CLI 路径、版本、help 和 App Server 能力检测；
- Codex Profile 创建、编辑、复制、删除、默认项、项目/分支绑定和导入导出；
- Skills/MCP 扫描与项目级绑定；
- 全局搜索、设置路径校验；
- Diagnostics、App Server probe、数据库检查、备份与恢复；
- 敏感字段清洗；
- Tauri NSIS 安装包构建；
- 公开 GitHub 仓库和 `main` 分支。

## 8. 已解决的重要性能事故

旧实现把每个原始 JSONL 完整内容同时放入两个 `detail_json`。真实数据库增长到约 2.22 GB 后，启动列表会反序列化接近 1 GB JSON，5 秒轮询还会反复读取和写入巨型记录，导致安装版打开后未响应。

`0.1.0-alpha.2` 已改为：

- 会话列表只读规范化摘要列；
- 会话详情从规范化子表重建；
- 新写入只保存紧凑元数据；
- 增量索引只解析新增字节；
- watcher 一次加载全部游标；
- 增加消息、工具和文件变化索引；
- 重型 Tauri command 异步执行；
- 前端轮询延迟、禁止重叠且最低 15 秒；
- 会话列表每批渲染 30 条；
- IPC 不再返回巨型 raw data。

真实旧数据库上的 alpha.2 启动约 2 秒并保持响应。旧数据库文件尚未自动压缩，后续需要安全迁移或压缩工具。

## 9. 当前 UI 路由

- `/projects`
- `/projects/:id/chat`
- `/projects/:id/continuation`
- `/projects/:id/context`
- `/sessions`
- `/sessions/:id`
- `/configurations`
- `/profiles`
- `/search`
- `/diagnostics`
- `/settings`

旧 AgentPack package 页面和后端模块仅作为 legacy/export 基础设施保留，不在主导航中。

### UI 与动效架构

当前操作模型是“动态档案编辑台”，不再保留常驻侧栏。顶部只承担品牌、当前空间、真实 re-index 和 Menu；全局 Menu 由两片以内侧为原点的深蓝 blade 旋转合拢，使用一条 paused/reversible timeline，快速点击只反转当前时间线，内容在面板覆盖后逐行揭示。

- `/projects` 是独立入口：大字 Continuum、真实最近会话和基于 BrandAppart Sticky Cards 思路的项目档案卡组。卡组通过 ScrollTrigger pin/scrub、3D tilt-off 与后层递进解释项目层级，只消费真实 Unified Project、绑定会话和 Context Health。
- 项目先进入可逆全屏概览 overlay，再通过重大切换进入全宽工作区。Chat、Sessions、Graph、Context、Activity、Files 共用持久舞台，使用 GSAP Flip 解释视图重排。
- 全局项目索引是底部抽屉；Context 是右侧检查器；Git、Skills/MCP、Diagnostics 是底部抽屉；Session Detail 和独立 Context 路由保持右侧档案页语义。
- 项目切换和全量会话扫描使用双层 10×11 block grid。遮罩完全覆盖后才启动真实操作，随后揭开原视图并以非阻塞状态条持续显示后端运行态；快速重复触发由全局锁拒绝。
- Fresh Continuation 的步骤文字只映射持久状态机，不生成百分比或模拟进度。
- 所有组件级 GSAP 使用 `@gsap/react` scope、清理和 overwrite/kill；`prefers-reduced-motion` 下跳过位移和块幕。

视觉规范与原始 Motion Prompt 对应关系见 `docs/ui/continuum-ui-spec.md`。

## 10. 代码导航

前端重点：`src/App.tsx`、`src/api/bridge.ts`、`src/store/appStore.ts`、`src/motion/ContinuumMotion.tsx`、`src/styles/continuum-ui.css`、`src/pages/ProjectsPage.tsx`、`src/pages/UnifiedChatPage.tsx`、`src/pages/NewContinuationPage.tsx`、`src/pages/ContextInspectorPage.tsx`、`src/pages/ProfilesPage.tsx` 和 `src/pages/DiagnosticsPage.tsx`。

Rust 重点：`src-tauri/src/commands.rs`、`database.rs`、`codex_adapter.rs`、`session_indexer.rs`、`unified_project.rs`、`context_compiler.rs`、`continuation.rs`、`codex_app_server.rs`、`codex_runtime.rs`、`profiles.rs`、`diagnostics.rs` 和 `git_inspector.rs`。

## 11. 当前未完成任务

### P0：优先完成

1. 更完整的会话链和 Conversation Graph 可视化。
2. 将已有分支比较和确定性选中节点合并后端接入 UI。
3. branch/Continuation 级 Skills/MCP 绑定、详情、依赖/重复警告、安全编辑与回滚。
4. 专用只读 Git 工作区 UI。
5. Context Health 提醒操作：立即 Fresh、稍后、忽略一次、项目关闭。
6. Raw Data 按页/按需读取接口，不能重新返回整个 JSONL。
7. 旧巨型数据库安全迁移、压缩和空间回收。
8. alpha.2 安装、首次启动、重启和绑定持久化验收更新。

### P1：P0 后实施

1. 自动轮换提醒和用户阈值；
2. 完整 Continuation 恢复中心；
3. 上下文历史检索；
4. 上下文冲突检测和交互解决；
5. Continuation 模板与预设；
6. Windows 托盘、后台扫描和系统通知；
7. 项目活动时间线和错误中心；
8. Continuum 项目安全导入导出；
9. 超大数据库、超长时间线性能画像和虚拟化。

多 Agent 真实适配应在 Codex P0/P1 稳定后推进，不得只增加枚举或按钮便声称支持。

## 12. 开发、测试和构建

Windows 依赖：Node.js 20+、Rust stable MSVC、Microsoft C++ Build Tools、Windows SDK、WebView2 Runtime、Git 和已登录的 Codex CLI。

```powershell
npm install
npm run tauri:dev
```

标准验证：

```powershell
npm run typecheck
npm test -- --run
npm run test:e2e
npm run build
cd src-tauri
cargo test --lib
cargo clippy --all-targets --all-features -- -D warnings
cd ..
npm run tauri:build
```

真实 Codex 验收会永久创建本地会话，默认忽略，只在明确需要时运行：

```powershell
cd src-tauri
cargo test --lib real_app_server_fresh_continuation_creates_binds_and_indexes_a_session -- --ignored --nocapture
```

2026-08-02 最近基线：TypeScript 通过；Vitest 9 文件/15 测试通过；Rust 45 通过、1 个真实测试忽略；Playwright 4 项通过；strict Clippy、Vite build、Tauri release 与 NSIS 通过。最终动态档案 UI 已在工作区 release WebView2 中实际启动，并以真实 SQLite v4 / 177 条本机会话复核首页、Overlay Menu 的中间态和完成态、Source Sessions 密度与 hover 对比度；浏览器另覆盖 reduced-motion 和无 Tauri bridge 的错误态。真实运行库没有 Unified Project，因此没有为视觉验收伪造项目；项目卡组数据态由新增组件测试验证。真实 Codex ignored 测试未重跑。

## 13. Git 和发布

- 远端：`https://github.com/YESNOPLEASEQI/Continuum.git`；
- 可见性：Public；默认分支：`main`；
- 首个公开提交：`edd102d`；
- 尚未添加开源许可证；公开可见不等于授予复制、修改或分发许可；
- release 二进制未提交，`src-tauri/target/` 被忽略。

## 14. 接手顺序

1. 阅读 `AGENTS.md` 和本文；
2. 执行 `git status -sb` 并保留用户修改；
3. 阅读任务相关代码和测试；
4. 选择可验收的纵向切片；
5. 使用临时数据库和隔离项目处理高风险测试；
6. 运行与风险相称的验证；
7. 更新 `AGENTS.md` 的状态、任务、验证和对话日志；
8. 长期架构或操作变化时同步本文；
9. 未经明确要求，不推送、发布、删除数据库或运行会永久创建会话的测试。

## 15. 已知风险

- 旧安装可能仍有约 2.22 GB 数据库；没有备份和空间检查时不能直接 `VACUUM`。
- App Server 协议随 Codex 版本变化，发布前必须重新探测能力和 Schema。
- App Server 生命周期通知已成为 Fresh 的直接持久化主路径；JSONL watcher 是只读校验/补漏层。高频 delta 依赖 `item/completed` 合并，进程在 item 完成前异常退出时由 JSONL 后备补齐。
- App Server 客户端请求 UI 已覆盖核心审批、权限申请、MCP elicitation 和工具用户输入；动态工具调用、认证令牌刷新、attestation 等不在当前产品职责内的请求仍显式失败。
- Raw Data 当前因性能安全收缩，后续必须分页，不能恢复巨型 IPC payload。
- `npm audit --omit=dev` 当前报告 React Router 7.18.2 的 RSC Mode CSRF advisory（2 个 high，来自 `react-router`/`react-router-dom`）；Continuum 不使用 RSC，但升级或降级到修复版本前需做路由回归，不能直接运行带 breaking change 的 `npm audit fix --force`。
- 页面和全局 CSS 较长，重构要小步验证；Vite 当前仍提示主 JS 约 550.75 kB，后续应按路由拆包。
- 动态档案编辑台已经替代第一轮保守 UI；后续不得恢复常驻侧栏，也不要把右侧 Context 与底部项目/工具抽屉重新堆成并列卡片栏。
- 工作区可能存在用户修改，任何 Agent 都必须先检查并保留。

## 16. 交接完成标准

- 请求已经完成或阻碍已准确说明；
- 必要验证已运行，未通过、未运行和受限测试明确区分；
- `AGENTS.md` 已记录本次变化、决策、验证和下一步；
- 长期事实变化时本文已同步；
- 没有提交数据库、会话原文、密钥或临时上下文；
- 没有把后端占位、UI 按钮或复制上下文误报为完整自动续接。
