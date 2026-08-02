# Continuum Agent Project Ledger

本文件是 Continuum 的**权威项目总账和所有后续 Agent 对话的强制工作约定**。任何在本仓库工作的 Codex/Agent 都必须在开始任务前完整阅读本文件，并在结束每一次项目相关对话前维护本文件。

详细交接说明见 `docs/HANDOFF.md`。如果本文与旧审计或旧聊天记录冲突，以当前代码和测试为第一事实，以本文的最新状态为第二事实。

## 1. 每次对话的强制协议

### 开始时

1. 完整阅读本文件。
2. 运行 `git status -sb`，不得覆盖、回滚或混入用户已有修改。
3. 阅读与任务直接相关的源码、测试和文档，不凭旧聊天记录猜测现状。
4. 判断请求是检查、诊断还是实施；诊断请求不自动扩大为修改。
5. 若代码事实与本文不一致，以代码为准，并在本次对话内修正本文。

### 工作中

1. 优先完成可真实验收的纵向功能，不只增加按钮、类型、枚举或 mock。
2. Fresh Continuation 永远不得用 Resume/Fork 冒充。
3. 长上下文写入项目内临时 Markdown，命令行只传短提示。
4. 不修改第三方原始 JSONL，不执行历史命令，不自动进行破坏性 Git 操作。
5. 不提交数据库、会话原文、密钥、环境变量值、`.continuum/`、构建产物或用户路径。
6. 性能敏感 IPC 不得传输完整巨型 JSONL；使用规范化列、分页和增量读取。
7. 改动数据模型必须提供版本化迁移、备份/恢复考虑和测试。

### 结束时

每一次项目相关对话都必须更新本文件，即使该对话只形成重要结论而没有改代码：

1. 更新“当前状态”中发生变化的版本、架构或能力。
2. 把完成项从任务队列移除或标记完成。
3. 记录真实执行的测试，不得把“预计通过”写成“通过”。
4. 在“对话交接日志”顶部追加记录，写清变化、决策、验证和下一步。
5. 如果改变长期架构、发布方式、数据模型或运行手册，同步更新 `docs/HANDOFF.md`。
6. 检查 `git diff` 和 `git status -sb`，说明是否存在未提交修改。

不要把逐条聊天原文复制进本文件。只记录能帮助下一个对话继续工作的事实、决策、证据和任务。

## 2. 当前项目快照

- 产品名：Continuum；目录名仍为 `AgentPackStudio`，不要因此把产品方向改回 AgentPack。
- 当前版本：`0.1.0-alpha.2`。
- 平台：Windows 11 x64，Tauri 2 桌面应用。
- 前端：React 19、TypeScript strict、Vite、React Router、Zustand、GSAP、`@gsap/react`。
- 后端：Rust 2021、bundled SQLite。
- 数据库 Schema：`v4`；v4 新增 App Server 通知、turn 和 item 的紧凑规范化持久化表。
- 真实 Agent 支持：Codex CLI/Desktop `0.146.0` 已验证；其他 Agent 只保留架构边界。
- GitHub：<https://github.com/YESNOPLEASEQI/Continuum>，Public，默认分支 `main`。
- 当前公开基线提交：`edd102d`（Initial public alpha release）。
- 本文件和 `docs/HANDOFF.md` 是基线提交之后新增的本地文档，是否推送由用户后续明确决定。
- 许可证：尚未添加。公开仓库目前不等同于已授予开源许可。

## 3. 产品目标

Continuum 管理本机 Codex 会话，把多个来源会话组织为 Unified Project、Conversation Branch 和统一时间线，并通过 Context Compiler 将旧长会话压缩为必要上下文，启动干净的新 Codex 会话，再自动绑定和持续监听。

第一核心流程：

```text
扫描真实 Codex 会话
  -> 选择并绑定 Unified Project / Branch
  -> 创建 Context Snapshot
  -> 编译必要上下文
  -> 写入 .continuum/continuations/*.md
  -> 在明确 cwd 启动干净 Codex thread
  -> 注入 CONTINUATION_ID + 短读取提示
  -> 获取或严格检测新 session ID
  -> 绑定回原项目和分支
  -> 增量导入消息
  -> 插入会话切换节点
```

三种操作必须在 UI 和实现上区分：

- Resume：继续原有长会话；
- Fork：从原历史分叉；
- Fresh Continuation：新建干净会话，只注入编译上下文，是默认主操作。

## 4. 不可违背的产品边界

- 不把任务包导入导出重新设为产品核心；旧 package 模块仅是 legacy/export 基础设施。
- 不使用 `codex resume`、`codex fork`、`thread/resume` 或 `thread/fork` 创建 Fresh。
- 不通过修改 Codex Desktop 安装目录或 UI 自动点击实现核心功能。
- Codex 深度集成优先使用公开 App Server；CLI + marker 检测是兼容后备。
- 不以“最新会话文件”作为唯一检测依据。
- 不将长上下文直接塞入命令行。
- 不声称能读取模型隐藏状态；只能使用公开的本地会话、工作区、Git 和配置数据。
- 不在生产 UI 注入演示会话或伪造成功状态。
- 没有真实解析、启动、检测、绑定和监听证据时，不得声称支持新的 Agent。

## 5. 当前已完成能力

### 本地会话与增量索引

- 扫描默认 `~/.codex/sessions` 和用户配置目录中的真实 JSON/JSONL；
- 损坏单行隔离，不让一个坏文件阻止全局扫描；
- 持久字节游标、半行处理、增量解析和重复 poll 去重；
- 规范化消息、工具调用、文件变化、错误、cwd 和时间元数据；
- watcher 批量加载游标，避免每文件单独查询数据库。
- App Server `thread/turn/item` 生命周期通知直接写入规范化会话、工具、文件变化和统一时间线；高频 token/output delta 合并到权威 `item/completed`，不逐块放大 SQLite 写入；JSONL watcher 通过 canonical item ID 复用进行去重校验和漏项补齐。
- Source Sessions 通过 session ID 只读关联 Codex `~/.codex/state_5.sqlite` 的 `threads.name/title/source`，优先显示与 Codex Desktop 侧边栏完全一致的标题并区分 Desktop/CLI；状态库缺失时才使用过滤协议注入后的首条真实请求兜底。列表仅展示标题、来源、项目、时间、绑定状态、Fresh 主操作和溢出菜单。

### Unified Project / Branch / Timeline

- 项目创建、重命名、归档、恢复、迁移和删除记录；
- 来源会话绑定、解绑、重新绑定和项目推荐；
- 统一时间线、来源追踪、搜索、过滤、分页、备注、复制和固定；
- 分支创建、切换、重命名、归档、恢复和安全删除；
- 分支比较和选中节点确定性合并后端已存在，UI 尚未接入完整流程。

### Context Compiler / Inspector / Health

- 确定性 Context Compiler V2；
- permanent、phase、short-term、retrieval 分层；
- `keep | compress | retrieve | exclude` 决策与理由；
- 项目目标、约束、决策、文件、TODO、失败、近期消息、受限工具日志、只读 Git 和项目级 Skills/MCP 摘要；
- Context Snapshot、内容 Hash、快照列表和 Diff；
- 部分 ContextItem 人工 override；
- Context Health 指标基础存在，完整提醒动作未完成。

### Fresh Continuation

- 持久状态机和非法迁移防护；
- 项目内上下文 Markdown 与 SHA-256；
- 唯一 `CONTINUATION_ID`；
- App Server `initialize`、`thread/start`、`turn/start`；
- App Server 命令执行、网络访问和文件修改审批请求 relay，全局 UI 可选择允许本次、本会话允许、拒绝或拒绝并停止；
- App Server `request_permissions`、MCP form/openai-form/URL elicitation 和工具用户输入请求已接入全局 UI，并按各自协议结构响应；
- App Server 通知流已成为 Fresh 会话消息、工具调用、文件变化和 turn 状态的直接持久化主路径；只保存紧凑生命周期账本和规范化字段，不保存完整通知 payload；
- App Server 握手通过隔离 fake 子进程覆盖乱序响应、协议错误、超时、提前退出和重复 thread ID；`thread/start` 返回的 ID 会在 `turn/start` 前检查，已被 Continuation 或项目绑定使用的会话会被拒绝，避免向旧会话注入 Fresh 上下文；
- 扩展请求保留原 JSON-RPC ID、按进程去重，`serverRequest/resolved` 或进程断开时清理；未知客户端请求显式返回 `-32601`；
- App Server 连接与审批队列由 Tauri 全局状态持有，服务端请求和同 ID 客户端响应不会混淆，未知客户端请求会收到显式错误而不是后台永久等待；
- App Server 条件不满足时 CLI 后备；
- CLI 候选按时间、cwd、marker、Agent 和未绑定状态严格匹配；
- 一个候选自动绑定，多个候选手工确认；
- 绑定后增量同步，重启后关系保留；
- 取消、重试、重新检测、手工绑定和临时文件清理基础；
- 真实 Codex App Server Fresh 验收已通过。

### Profiles / Config / Diagnostics

- Codex executable、版本、help、Resume/Fork/App Server 能力检测；
- Profile 创建、编辑、复制、删除、默认项、项目/分支绑定和导入导出；
- Skills/MCP 扫描和项目级绑定；
- 全局搜索、设置路径校验；
- Diagnostics 和 App Server probe；
- 数据库 integrity check、备份和恢复；
- 诊断和解析数据的敏感字段清洗。

### UI / Motion

- 全局已取消常驻侧栏，顶部只保留品牌、当前空间、真实 re-index 状态和 Menu；Menu 使用双片内侧旋转 blade、同一条可逆 GSAP timeline、逐行 mask reveal 和焦点陷阱，快速反向点击不会堆积动画。
- `/projects` 是独立首页：字符级 Continuum 入口、真实最近会话，以及基于 BrandAppart 思路的 ScrollTrigger 项目档案卡组；卡片只读取 Unified Project、绑定会话和 Context Health，不在生产环境注入演示数据。
- 选择项目先打开可逆项目概览 overlay，再进入全宽 Unified Project 工作区；Chat、Sessions、Graph、Context、Activity、Files 使用 GSAP Flip 解释持久区域的真实位置/比例变化，Conversation Graph 的完整比较/合并流程仍未完成。
- 项目索引改为底部抽屉；Context 使用右侧检查器，Git、Skills/MCP、Diagnostics 使用底部抽屉；深层 Context/Session 路由保留为右侧档案页，窄窗口退化为全宽面板。
- 项目切换与全量扫描使用双层 10×11 block grid 和全局重入锁；遮罩覆盖后启动真实操作，随后揭开原视图，以非阻塞状态条持续显示真实 RUNNING 状态。
- Fresh Continuation 动画只响应后端持久状态机，不伪造百分比；所有 GSAP 动画支持清理、overwrite/kill 和 `prefers-reduced-motion`。
- 设计来源和页面流见 `docs/ui/continuum-ui-spec.md`。

### 桌面与发布

- Tauri NSIS 构建；
- alpha.2 已修复真实巨型数据库上的启动未响应；
- 公开 GitHub 仓库已经建立；
- release 二进制未纳入 Git。

## 6. Fresh 和 Profile 规则

Profile 是启动配置预设，不是会话内容。它可包含模型、推理强度、审批、沙箱、Codex config、启动通道和额外参数。

- Fresh：主界面必须允许选择；
- Fork：允许选择；
- Resume：默认保持原会话，覆盖项仅放高级设置；
- 逻辑分支：可记录默认 Profile，真正启动时再次确认。

继承顺序：

```text
全局默认 -> 项目默认 -> 分支默认 -> 本次启动
```

本次选择不能静默改写上层默认。每次启动最终要保存不可变 Profile 快照。

App Server 在 Profile 可无损映射且没有 CLI 专用参数时可承载 `never`、`on-request` 和 `untrusted`。命令执行、网络上下文、文件修改、`request_permissions`、MCP elicitation 和工具用户输入均通过全局 UI relay。权限申请只允许返回原请求的子集；MCP 表单数据和工具回答仅在内存中保留到响应完成。其他未知客户端请求显式返回“不支持”错误，不能静默永久等待。

## 7. 重要性能事实

旧数据库曾达到约 2.22 GB。根因是完整原始 JSONL 被重复存入两个 `detail_json`，列表和 5 秒轮询反复反序列化巨型 JSON，造成安装版打开后未响应。

alpha.2 已改为规范化摘要查询、紧凑元数据、增量字节解析、索引、异步 command、非重叠最低 15 秒轮询和每批 30 条渲染。真实旧数据库启动约 2 秒并保持响应。

后续禁止恢复：完整 raw JSONL IPC、每次 poll 全文件重读、初次渲染加载所有数据、每个文件单独连接数据库。

旧数据库仍需要安全迁移/压缩。执行 `VACUUM` 前必须备份、检查可用磁盘空间，并考虑 SQLite 重建所需临时空间。

## 8. 当前主要入口

前端路由：`/projects`、`/projects/:id/chat`、`/projects/:id/continuation`、`/projects/:id/context`、`/sessions`、`/sessions/:id`、`/configurations`、`/profiles`、`/search`、`/diagnostics`、`/settings`。

重点源码：

- `src/api/bridge.ts`
- `src/pages/UnifiedChatPage.tsx`
- `src/pages/NewContinuationPage.tsx`
- `src/pages/ContextInspectorPage.tsx`
- `src/pages/ProfilesPage.tsx`
- `src-tauri/src/database.rs`
- `src-tauri/src/session_indexer.rs`
- `src-tauri/src/context_compiler.rs`
- `src-tauri/src/continuation.rs`
- `src-tauri/src/codex_app_server.rs`
- `src-tauri/src/codex_runtime.rs`
- `src-tauri/src/unified_project.rs`

## 9. 当前任务队列

### P0 — 必须先完成

- [ ] 完整 Conversation Graph / session-chain 可视化。
- [ ] 分支比较和确定性选中节点合并 UI。
- [ ] branch/Continuation 级 Skills/MCP 绑定、详情、依赖/重复警告、安全编辑和回滚。
- [ ] 专用只读 Git workspace UI。
- [ ] Context Health 提醒动作和按项目禁用/稍后策略。
- [ ] Raw Data 按页/按需读取，不恢复巨型 IPC。
- [ ] 旧巨型 SQLite 安全迁移、压缩和空间回收。
- [ ] alpha.2 安装、启动、重启、绑定持久化的桌面验收更新。

### P1 — P0 后完成

- [ ] 自动轮换提醒和用户阈值。
- [ ] 完整 Continuation 恢复中心和日志。
- [ ] 上下文历史检索、固定、过期和错误标记。
- [ ] 上下文冲突检测与解决。
- [ ] Continuation 模板和可复用预设。
- [ ] Windows 托盘、后台扫描和系统通知。
- [ ] 项目活动时间线与错误中心。
- [ ] Continuum 项目安全导入导出。
- [ ] 超大 session store、超长 timeline 性能画像和虚拟化。

### P0/P1 后

- [ ] 评估并真实实现其他 Agent Adapter；不得只增加占位。
- [ ] 决定开源许可证。
- [ ] 建立 CI、release 附件、签名和更新策略。

## 10. 验证基线和命令

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

真实 Codex 测试会永久创建本地会话，只在明确需要时运行：

```powershell
cd src-tauri
cargo test --lib real_app_server_fresh_continuation_creates_binds_and_indexes_a_session -- --ignored --nocapture
```

最近基线：

- 2026-08-02 `npm run typecheck`：通过；
- 2026-08-02 `npm test -- --run`：9 文件、15 测试通过；
- 2026-08-02 `cargo test --lib`：45 通过、0 失败、1 个真实 Codex 测试忽略；
- 2026-08-02 `npm run test:e2e`：4 通过；
- 2026-08-02 strict Clippy、Vite production build、Tauri release、NSIS：通过。

“最近”结果在相关代码发生变化后会失效。修改者必须运行与风险相称的测试并更新这里。

## 11. 文档地图

- `docs/HANDOFF.md`：详细交接手册；
- `docs/product-definition.md`：产品边界；
- `docs/architecture.md`：架构和 Fresh 设计；
- `docs/developing-with-chatgpt-codex-client.md`：App Server 调研和集成边界；
- `docs/fresh-continuation-acceptance-2026-08-01.md`：真实 Fresh 验收证据；
- `docs/security.md`：安全约束；
- `docs/remaining-work.md`：短版当前任务；
- `docs/full-development-audit.md`：早期历史审计，部分状态已过期；
- `docs/nightly-worklog.md`：历史开发日志。

## 12. 对话交接日志

在本节顶部追加记录。保留最近约 20 条高价值记录；更旧内容可整理进 `docs/nightly-worklog.md`。格式必须包含日期、目标、变化、验证、未完成和 Git 状态。

### 2026-08-02 — 动态档案编辑台全量前端操作模型落地

- 目标：按用户选定的方案 2 完整重构 Continuum 前端交互，不再从旧常驻侧栏和保守动效继续演化，并在真实 SQLite 数据下复查视觉结果。
- 变化：重写最终规格 `docs/ui/continuum-ui-spec.md`；移除常驻侧栏，加入双 blade 全屏 Overlay Menu；首页改为大字档案入口、真实最近会话和 ScrollTrigger/3D 项目卡组；项目概览改为全屏档案 overlay；Unified Project 改为全宽工作区，Chat/Sessions/Graph/Context/Activity/Files 使用 Flip，项目索引落到底部，Context 在右侧，Git/Skills/MCP/Diagnostics 在底部。重大导航继续使用 block grid；Fresh 只展示持久状态机真实步骤；全部 GSAP 使用 scope 清理、overwrite/kill、可逆 timeline 和 reduced-motion 分支。真实桌面复查后修正 Source Sessions 浅色 hover 上的标题/元数据对比度，并修复 Search 自动聚焦被 AppShell 路由焦点覆盖的问题。
- 原始参考：完整读取 Motion Prompts 的 `contact-form`、`project-page-overlay-animation`、`creative-clutter`、`sidebar-slide-out-menu`、`page-transitions`，并进一步读取 Audemars Piguet Overlay Menu 与 BrandAppart Sticky Cards 原始 Prompt；最终以后二者为全局菜单和首页卡组主骨架，其余参考按真实产品语义组合使用。
- 验证：最终状态 `npm run typecheck` 通过；Vitest 9 文件/15 测试通过；Vite production build 通过；Playwright 4 项通过（另定向重跑 navigation 1 项通过）；`cargo test --lib` 45 通过、0 失败、1 个会永久创建真实 Codex 会话的测试忽略；strict Clippy 通过；最终 `npm run tauri:build`、release EXE 和 NSIS 通过。工作区 release 在真实 SQLite v4 / 177 条本机会话下实际启动，检查首页、双 blade 菜单中间态/完成态、Sessions 密度与 hover 修复后再次启动复验。NSIS 4,093,435 bytes，SHA-256 `4057FF01FE7E10327FB54339873E32791D4C24694D8844C90DC10C6E7E186120`。
- 未完成：真实运行库当前没有 Unified Project，故项目卡组多卡滚动、项目概览和项目工作区的数据态由组件测试/E2E 与现有真实业务流验证，未为视觉验收伪造或创建项目；Conversation Graph 合并 UI、branch/Continuation Skills/MCP、专用 Git 工作区、Context Health 操作等原 P0 仍在队列。Vite 仍提示主 JS 约 550.75 kB 超过 500 kB，后续应做路由级 code splitting。ignored 真实 Codex 测试未运行。
- Git 状态：`main` 跟踪 `origin/main`；本轮 UI、测试和文档与此前 App Server/Schema v4 等本地修改均未提交、未推送；未覆盖或回滚用户已有修改，构建产物保持忽略。

### 2026-08-02 — 两套全量 UI 视觉方向待选

- 目标：在正式重构前提供两套真正不同的 Continuum 视觉与交互系统，避免再次把 Motion Prompt 当成旧界面的局部装饰。
- 变化：使用 Product Design、Visualize 与 frontend-design 工作流重新核对现有产品语义，并通过内置浏览器实际打开 Motion Prompts 的 Audemars Piguet 菜单与 BrandAppart 卡片参考素材；两次 ImageGen 均因 `chatgpt.com/backend-api/codex/images/edits` 网络错误失败，因此按用户允许改为提供两套通俗语言方案供选择。本轮不修改生产前端。
- 决策：两套方案都取消常驻侧栏并保留独立首页、全屏结构化 Menu、项目/最近会话主入口和真实 Fresh 状态，但在首页信息组织、工作区空间模型、检查器关系、颜色/字体与动效语法上必须明显不同；选择前不进入生产实现。
- 验证：确认并截图打开 `https://motionprompts.dev/c/audemarspiguet-menu/hero.jpg` 与 `https://motionprompts.dev/c/brandappart-sticky-cards/card-img-1.jpg`；两次图片生成请求均记录为网络失败；未运行代码测试，因为没有生产代码改动。
- 未完成：用户尚未选择方案 1 或方案 2；选择后需更新 `docs/ui/continuum-ui-spec.md` 为最终规格，再完整重构所有主要前端流程并按基线执行类型检查、测试、构建与桌面视觉验收。
- Git 状态：`main` 跟踪 `origin/main`；仅新增本条总账记录，既有第一轮 UI、App Server/Schema v4 与其他本地修改仍未提交、未推送，未覆盖或回滚任何用户改动。

### 2026-08-02 — UI 全面重构方向纠偏（仅完成理解）

- 目标：纠正第一轮把 Motion Prompt 保守附着在旧侧栏/旧路由结构上的误解，明确下一轮是完整重构所有前端操作方式，而不是继续微调现有样式。
- 结论：软件启动先进入独立首页；默认取消常驻侧栏；全局 Menu 采用 `audemarspiguet-menu` 原始 Prompt 的双半屏旋转门（内侧轴、初始 ±180°/2× scale、`hop` CustomEase）、汉堡到 X、同一 paused timeline 正反播放与 masked-line stagger；首页使用 `brandappart-sticky-cards` 原始 Prompt 的 pinned deck、透视、逐段 scrub、前卡向上并 rotationX 35° 退场、后卡逐级前移放大，承载项目与最近会话。刷新 block grid 和项目黑白概览可保留为候选语言，其余廉价/不协调动效应删除并重新设计。项目内 Chat、Sessions、Graph、Context、Activity、Files、Fresh、Git、Skills/MCP、Diagnostics 的操作模型和信息架构全部重新推导，不能把当前侧栏/右侧面板当作固定前提。
- 决策：第一轮 `docs/ui/continuum-ui-spec.md` 已标记为被否决的探索，不得继续作为最终 UI 目标；生产代码本轮未修改，下一步必须先做完整视觉方向选择，再实施。Product Design 没有已保存的用户上下文，本轮只使用当前代码、用户反馈和两份原始 Motion Prompt。
- 验证：完整阅读 `audemarspiguet-menu/prompt.md` 与 `brandappart-sticky-cards/prompt.md`；重新读取 frontend-design、Product Design 路由/get-context/ideate 与 Visualize 指令；未运行代码测试，因为本轮没有业务代码改动。
- 未完成：尚未生成并选择新的完整视觉方向，未删除现有侧栏或修改任何生产 UI；下一轮不能直接开工，需先把首页、Overlay Menu、Sticky Cards 和项目工作区作为同一个空间系统完成视觉定稿。
- Git 状态：`main` 跟踪 `origin/main`；仅维护总账、交接和旧规格的失效标记；第一轮 UI、此前 App Server/Schema v4 等全部本地修改仍未提交、未推送，未覆盖或回滚既有改动。

### 2026-08-02 — Continuum 专业桌面 UI 与 Motion 系统重构

- 目标：在不改变 App Server、CLI fallback、Unified Project/Timeline、Context Compiler、Fresh Continuation 和 SQLite 业务边界的前提下，建立有辨识度且可解释空间关系的多会话桌面 UI。
- 变化：motionprompts MCP 未暴露可调用工具或资源，因此完整读取本机 `motionprompts-mcp` 的 `contact-form`、`project-page-overlay-animation`、`creative-clutter`、`sidebar-slide-out-menu`、`page-transitions` 五份原始 `prompt.md`，并写入 `docs/ui/continuum-ui-spec.md`。新增 GSAP/`@gsap/react` motion 层、双层 10×11 重大操作过渡和统一扫描 hook；`/projects` 改为字符 reveal 入口、真实项目索引和可逆概览；Unified 工作区新增 Chat/Sessions/Graph/Activity/Files 视图与 Flip 重排、Context/Git/Skills/Diagnostics 右侧检查器；Fresh 页面按真实持久状态展示执行说明。视觉改为档案纸、石墨工作台、朱红信号和遥测绿，不使用渐变或圆角卡片网格。
- 迭代：真实 2.70GB 级历史数据扫描持续超过一分钟，首轮全屏 RUNNING 遮罩会阻塞客户端；现改为覆盖确认启动后揭开原视图，以底部非阻塞 RUNNING 状态条继续显示真实操作，扫描期间可正常切换页面。重复触发由全局锁拦截，reduced motion 跳过位移和块幕。
- 验证：最终 `npm run typecheck` 通过；`npm test -- --run` 8 文件/14 测试通过；`npm run build` 通过；Playwright 4 通过；此前同轮 `cargo test --lib` 45 通过、0 失败、1 个真实 Codex 测试忽略，strict Clippy 通过；最终 `npm run tauri:build` 生成 release EXE 与 NSIS。工作区 release WebView2 实际启动，约 176 条本机会话数据上检查入口、cover、非阻塞 RUNNING 和扫描期间导航；1626×990、900×720 与 reduced-motion 浏览器布局无横向溢出。NSIS 4,072,731 bytes，SHA-256 `693C1BA3878FD0872699A3053069659C3E8BF4C9105234BA54A70DCE2FB8FE0`。
- 未完成：真实运行数据没有 Unified Project，故项目概览和工作区数据态由组件测试/浏览器路径验证，未在该 release 数据库中创建临时项目；完整 Conversation Graph 合并 UI、专用 Git 工作区、branch/Continuation Skills/MCP 管理等原 P0 仍在队列。被主动终止的全量扫描只用于验证长任务 UI，未等待 2.70GB 历史库完全重建；未运行会永久创建会话的 ignored 真实 Codex 测试。`npm audit --omit=dev` 仍报告 React Router 7.18.2 的 RSC Mode advisory（2 个 high）；当前桌面应用不使用 RSC，未冒险执行会强制降级的 `npm audit fix --force`。
- Git 状态：`main` 跟踪 `origin/main`；本轮 UI、依赖、测试与文档修改和此前 App Server/Schema v4 等本地修改均未提交、未推送；未覆盖或回滚既有改动，构建产物保持忽略。

### 2026-08-02 — 与 Codex Desktop 标题完全一致并完成真实桌面验收

- 目标：纠正“自行从首条请求生成标题”的需求误解，改为显示与 Codex Desktop 侧边栏完全一致的正式会话标题，并在真实安装数据上验收。
- 变化：定位 Codex 公开本地状态库 `~/.codex/state_5.sqlite`，按 session/thread ID 只读关联 `threads`，标题优先级为非空 `name` 后 `title`，来源 `vscode` 映射为 Desktop、`cli/exec` 映射为 CLI；状态库不可用时才回退到规范化消息首条真实请求，无真实请求且旧标题为 rollout/协议标签时显示“未命名会话”。不修改 Codex 状态库，不新增 Continuum Schema，不读取巨型 detail JSON。
- 验证：真实状态库核对 `019fc1f0-71c4-7431-95d0-ba2249e3bfdb` 的 Codex 标题为“优化 Source Sessions UI”、source 为 `vscode`；构建 release 后关闭旧安装进程，启动工作区 release 可执行文件并用 Windows Graphics Capture 实际检查 2.70 GB 活动数据库页面，首行真实显示“优化 Source Sessions UI”和“Codex Desktop”，其他可见行显示 Codex 自身标题（如“创建 Karpathy LLM 知识库”“专业评价项目”），不再显示 rollout 或推荐插件标签。`cargo test --lib` 45 通过、0 失败、1 个真实 Codex 测试忽略；strict Clippy、Vite/Tauri release、NSIS 通过。安装包 4,020,187 bytes，SHA-256 `ADB39D5065309D6B0DCA0C456178CE2C089B6CCED660008BCC9C879757170530`。
- 未完成：当前实际验收运行的是工作区 release 可执行文件；用户仍需运行同一构建生成的 NSIS 覆盖安装，安装动作未由 Agent 代执行。Codex state schema 属于外部公开本地实现，后续 Codex 升级若更改表结构需优雅回退到首条真实请求。
- Git 状态：`main` 跟踪 `origin/main`；本轮标题数据源修复、测试和此前全部本地改动仍未提交、未推送；构建产物保持忽略。

### 2026-08-02 — 旧索引标题/来源增量修复与安装包重建

- 目标：修复安装后 Source Sessions 仍显示 rollout 或后续追问标题、来源仍为中性 Codex 的真实运行时问题，并重新交付覆盖安装包。
- 变化：核对真实 Codex Desktop JSONL 与活动 2.70 GB 数据库，确认全量解析字段正确，但旧索引在增量更新时只检查本次新增消息，会把后续追问误作首条请求；增量流又从文件尾读取，无法看到首行 `session_meta.originator`。现在旧标题需要修复时会从规范化 `session_messages` 最早 user 记录中选首条真实请求（最多读取前 100 条 user 消息，不读取巨型 detail JSON），来源缺失时只读取 JSONL 首条 session metadata；新增真实缺陷回归测试。
- 验证：真实活动数据库中问题 session 曾被错误更新为后续追问且 `clientKind=unknown`，与根因一致；定向新回归测试通过；完整 `cargo test --lib` 44 通过、0 失败、1 个真实 Codex 测试忽略；strict Clippy 通过；`npm run tauri:build` 完整通过并生成新的 NSIS。新安装包 4,025,275 bytes，SHA-256 为 `B93784392AD1E75B69893EE55C93236DAA869F7470BF4CB6810CE6C9A61E6B47`。
- 未完成：用户需覆盖安装新包并点击一次“扫描 Codex”，让现有旧索引立即全量刷新；尚未替用户执行安装后的 UI 验收。
- Git 状态：`main` 跟踪 `origin/main`；修复代码、测试及此前全部本地改动仍未提交、未推送；构建产物位于被忽略的 `src-tauri/target/`。

### 2026-08-02 — Source Sessions 修改版 NSIS 构建

- 目标：生成包含本轮 Source Sessions 来源、标题和极简列表修改的可安装 Windows 版本。
- 变化：未再修改业务代码；成功生成 `Continuum_0.1.0-alpha.2_x64-setup.exe`，构建产物仍位于被忽略的 `src-tauri/target/`，未纳入 Git。
- 验证：`npm run tauri:build` 完整通过，包含 TypeScript、Vite production、Rust release、Tauri bundle 与 NSIS；安装包 4,013,786 bytes，SHA-256 为 `8EC7702BDC0294E9D79A6D9FCFA69B78EC451FD91E89E2BA107E9275922B3536`。未执行安装后的启动与重新扫描验收。
- 未完成：用户仍需运行安装包并在 Source Sessions 点击“扫描 Codex”，让旧索引刷新来源和人类标题；其他 P0 保持原队列。
- Git 状态：`main` 跟踪 `origin/main`；构建未改变已跟踪源码状态，本轮代码、App Server、Schema v4、审批 UI 与文档仍为本地未提交修改，未推送。

### 2026-08-02 — Source Sessions UI、来源与人类标题实施

- 目标：按 Figma 审计板完成 Source Sessions 的 Desktop/CLI 来源识别、协议注入过滤、人类标题和极简列表。
- 变化：Codex 解析器读取 `originator/client` 并归一为 Desktop、CLI 或 unknown；标题与 goal summary 跳过以 `<recommended_plugins>`、`<environment_context>`、`<app-context>`、权限/协作/技能等协议标签开头的伪 user 内容，使用首条真实用户请求，增量索引也可替换旧的协议标签或 rollout 文件名标题。来源写入现有 `source_sessions.raw_metadata` 紧凑字段，列表查询通过现有绑定表返回项目名与绑定状态，不新增 Schema、不读取巨型 `detail_json`。前端行内仅保留标题、来源、项目/工作区短名、时间和绑定状态；唯一主按钮改为“新建续接”，Resume、Fork、详情和复制路径进入 `···` 菜单；删除 Session ID、完整路径、消息/工具计数和“其他 Agent：未来扩展”。已绑定会话无需再次选择项目即可进入 Fresh。
- 决策：无法可靠识别来源时显示中性的 `Codex`，不再误报为 CLI；旧索引需要下一次真实扫描才能从 JSONL 补齐来源与新标题。项目辅助信息优先显示绑定项目名，未绑定时显示工作区末级目录名。
- 验证：Figma `Continuum · Source Sessions UI Audit` 整板截图核对；`npm run typecheck` 通过；`npm test -- --run` 8 文件/14 测试通过；`npm run build` 通过；`npm run test:e2e` 3 通过；`cargo test --lib` 43 通过、0 失败、1 个真实 Codex 测试忽略；strict Clippy 通过；随后 `npm run tauri:build` 完整通过并生成 NSIS。未运行会永久创建真实 Codex 会话的 ignored 测试。
- 未完成：其他 P0 保持原队列；尚未在安装版真实 2.22 GB 旧库上做本轮视觉验收，运行实例需重新扫描一次以刷新旧来源和标题。
- Git 状态：`main` 跟踪 `origin/main`；本轮修改与此前 App Server、Schema v4、审批 UI 和文档修改均为本地未提交修改，未推送；未覆盖或回滚既有改动。

### 2026-08-02 — Source Sessions UI 与命名审计

- 目标：根据真实 Source Sessions 截图识别全部可见 UI 问题，并按“人类能看懂的标题和最小功能信息”收敛目标。
- 变化：未修改代码；确认所有 `AgentKind::Codex` 被前端静态显示为 `Codex CLI`，但真实会话 `session_meta.originator` 为 `Codex Desktop`，当前 `SessionSummary` 未携带客户端来源。标题算法直接取第一条 user 消息，导致系统注入的 `<recommended_plugins>`、`<environment_context>` 被当作标题；无有效用户消息时退回 `rollout-时间-UUID` 文件名。列表还重复显示 Session ID、完整本机路径、消息/工具计数、三项并列操作、无功能的“其他 Agent：未来扩展”和复制图标，信息密度高且主流程不清晰。
- 决策：默认列表只应显示经协议内容过滤后的首条真实人类请求标题，辅助信息限制为真实来源（Codex Desktop/CLI）、工作区短名/绑定项目和更新时间；Fresh Continuation 是唯一主操作，Resume/Fork/复制路径进入溢出菜单。Session ID、完整路径和计数仅在详情或诊断中显示；未来 Agent 占位不得逐行出现。
- 交付：创建 Figma 审计板 `Continuum · Source Sessions UI Audit`（<https://www.figma.com/design/NZDyLIORAXqFS6T61hQGd9>），左侧保留并标注原始界面截图，右侧给出极简会话行和旧版到新版映射，下方记录来源、标题、列表三项最小实现范围。
- 交接：已为后续独立 UI 实施对话准备简短提示词，要求先读总账和交接文档，再按上述 Figma 方案修正来源、标题与列表信息密度，不扩大到其他 P0 功能。
- 验证：按原始 1626×990 截图检查；对照 `SessionsPage.tsx`、`agents.ts`、`codex_adapter.rs`、`SessionSummary` 模型和 CSS；抽查截图首条真实 JSONL，确认 `originator=Codex Desktop`，第一条 user message 是 `<recommended_plugins>`，实际人类请求在其后。Figma 最终整板截图已人工检查，原截图使用 IMAGE fill，41 个文本节点无零宽，顶层节点无越界。本轮未修改业务代码，未运行测试。
- 未完成：尚未实施标题过滤、来源字段、列表精简和交互调整；截图无法验证键盘顺序、屏幕阅读器名称、hover/focus、缩放与窄屏重排，实施后需补组件测试和桌面视觉验收。
- Git 状态：`main` 跟踪 `origin/main`；本轮无代码改动，仅维护总账和任务队列；此前 App Server、Schema v4、审批 UI、可靠性测试和文档仍为本地未提交修改，未推送。

### 2026-08-02 — 客户端会话检测与“继承”能力核查

- 目标：确认当前实现是否仍无法检测 Codex 客户端对话并进行继承。
- 变化：未修改代码；核对默认路径、扫描器、Continuation 绑定与真实验收代码。当前实现会扫描 `~/.codex/sessions` 中 Codex Desktop/CLI 的公开 JSON/JSONL，会话可绑定到 Unified Project，并通过 Context Compiler 生成可解释快照后启动 Fresh Continuation；这里的“继承”是公开消息、工具、文件、Git、约束和配置摘要的上下文继承，不是模型隐藏状态或旧历史的无损内存复制。
- 验证：本机默认 Codex sessions 目录存在且当前含 173 个 JSON/JSONL 文件；还发现 Roaming 应用数据与 Codex 包的 LocalCache 下各有 Continuum 数据库路径，空列表需要优先核对实际运行包、数据库位置、设置中的 session path 和扫描状态。本轮未运行测试。
- 未完成：尚未针对用户当前看到的具体页面/安装包读取 Diagnostics 或数据库索引数量，因此不能仅凭源码断定其运行时失败点；若当前 UI 仍为空，下一步应检查该运行实例的 Settings 与 Diagnostics，而不是重新实现扫描器。
- Git 状态：`main` 跟踪 `origin/main`；本轮无代码改动，仅维护本总账；此前 App Server、Schema v4、审批 UI、可靠性测试和文档仍为本地未提交修改，未推送。

### 2026-08-02 — fake App Server 可靠性闭环

- 目标：完成 P0 中 App Server 错误响应、超时、提前退出、乱序响应和重复 thread ID 的隔离集成测试，并修复测试揭示的 Fresh 会话复用风险。
- 变化：加入 Windows 临时 PowerShell fake App Server 子进程夹具，真实走进程启动、stdio JSONL、握手、超时终止和错误传播；生产握手允许测试注入短超时但默认仍为 20 秒。`thread/start` 返回 ID 后、`turn/start` 注入上下文前会查询 Continuation 与项目绑定，已使用 ID 会终止子进程并按无可绑定 partial thread 的启动失败返回，避免污染旧会话。fake 进程测试使用全局测试锁串行化，消除并行 PowerShell 启动造成的耗时抖动。
- 决策：重复 thread ID 不是可继续绑定的 partial thread；只有已经确认创建全新 thread、但初始 `turn/start` 失败时才保留 partial thread 供用户恢复，Fresh 不得自动向任何已使用会话重试注入。
- 验证：定向 fake App Server 测试 4 通过；首次完整 `cargo test --lib` 因并行 fake 进程导致“总耗时小于 2 秒”断言抖动失败，串行化夹具后完整测试 42 通过、0 失败、1 个真实 Codex 测试忽略；`cargo clippy --all-targets --all-features -- -D warnings` 通过；`cargo fmt --all -- --check` 通过。未运行会永久创建真实 Codex 会话的 ignored 测试，也未重跑前端、Playwright、Vite 或 Tauri/NSIS 构建。
- 未完成：其余 P0 产品界面、旧巨型 SQLite 压缩以及 alpha.2 安装/重启/绑定持久化桌面验收仍在队列。
- Git 状态：`main` 跟踪 `origin/main`；本轮可靠性代码与此前 App Server、Schema v4、审批 UI、总账和交接文档均为本地未提交修改，未推送；构建产物保持忽略。

### 2026-08-02 — App Server 通知流直接持久化

- 目标：完成 P0 中 App Server 通知直接消费/持久化，让 JSONL watcher 降级为去重校验与漏项补齐层。
- 变化：新增数据库 Schema v4 及 `app_server_notifications`、`app_server_turns`、`app_server_items` 紧凑表；reader 直接处理 `thread/started`、turn、item 生命周期和错误通知，把 user/agent message、命令/MCP/动态工具、文件变化同步投影到现有会话表与 Unified Timeline。高频 token、reasoning、output、patch delta 不逐条写库，使用权威 `item/completed` 合并结果；单个工具字段限制为 256K 字符，file change 不保存 diff。JSONL 全量和增量索引会按 role/content 或工具身份复用 App Server canonical item ID，并用 App Server 数据补回 JSONL 缺项，避免双重时间线节点。迟到的 started 不会覆盖 completed。
- 决策：通知账本只保存 hash、method、thread/turn/item ID 和时间，不保存完整协议 payload；App Server 是即时规范化主路径，第三方 JSONL 保持只读并作为 verifier/fallback。v2/v3 升级到 v4 前仍创建可恢复 SQLite 备份，Windows 上先检查“数据库大小 + 128 MiB”可用空间，复制失败会清理半成品并中止迁移。
- 验证：`npm run typecheck` 通过；`npm test -- --run` 8 文件/14 测试通过；`cargo test --lib` 38 通过/1 个真实 Codex 测试忽略；strict Clippy 通过；`npm run build` 通过；Playwright 3 通过；加入 Schema v4 和 Windows 磁盘空间检查依赖后的最终 `npm run tauri:build` 通过，生成 release EXE 和 NSIS。未运行会永久创建真实 Codex 会话的 ignored 测试。
- 未完成：fake App Server 的超时、退出、错误响应和重复 thread ID 集成测试仍在 P0；本轮覆盖重复通知、迟到 started、消息/工具/文件投影、JSONL canonical ID reconciliation，以及 v2/v3 到 v4 的备份迁移。
- Git 状态：`main` 跟踪 `origin/main`；本轮与此前审批 relay、总账和交接文档均为本地未提交修改，未推送；构建产物保持忽略。

### 2026-08-02 — App Server 扩展客户端请求闭环

- 目标：完成 P0 中 `request_permissions`、MCP elicitation 和工具用户输入的真实 App Server UI/响应闭环。
- 变化：按本机 `codex-cli 0.146.0` 生成的 ServerRequest Schema 实现 `item/permissions/requestApproval`、`mcpServer/elicitation/request` 和 `item/tool/requestUserInput`；全局对话框可展示权限、MCP form/openai-form/URL、选项与自由回答，并分别返回权限子集与 turn/session scope、MCP action/content、按问题 ID 的 answers。App Server 初始化显式协商 `experimentalApi` 与 `mcpServerOpenaiFormElicitation`；原 RPC ID 保留，重复请求去重，`serverRequest/resolved` 和进程断开均清理内存请求；未知请求继续显式失败。
- 决策：扩展请求与已有审批共用进程级内存队列，但使用按 kind 校验的类型化响应，不能把所有请求伪装成 `decision`；权限响应不得包含原请求之外的能力；秘密回答和 MCP 表单内容不持久化。
- 验证：`npm run typecheck` 通过；`npm test -- --run` 8 文件/14 测试通过；`cargo test --lib` 36 通过/1 个真实 Codex 测试忽略；strict Clippy 通过；`npm run build` 通过；Playwright 3 通过；`npm run tauri:build` 生成 release EXE 和 NSIS 成功。未运行会永久创建真实 Codex 会话的 ignored 测试。
- 未完成：App Server 通知流持久化、fake App Server 超时/退出/乱序/错误响应和重复 thread ID 集成测试仍在 P0；本轮只覆盖请求重复 ID 与 resolved 清理的单元测试。
- Git 状态：`main` 跟踪 `origin/main`；本轮与此前审批 relay、总账和交接文档均为本地未提交修改，未推送；构建产物保持忽略。

### 2026-08-02 — App Server 核心审批 relay

- 目标：继续 P0，解除 `on-request` / `untrusted` Profile 只能回退 CLI 的限制，并避免后台审批不可见地悬挂。
- 变化：新增进程级 App Server 可写连接和内存审批队列；识别命令执行、网络上下文和文件修改审批请求；通过 Tauri 查询/响应命令和全局可聚焦对话框展示上下文并写回 `accept | acceptForSession | decline | cancel`；服务端请求与同 ID 客户端响应明确区分；未知客户端请求返回 JSON-RPC `-32601`；无 CLI 专用参数的三种 Approval Mode 均可走 App Server。
- 决策：审批只在对应 App Server 子进程存活期间保留，不写入 SQLite；进程断开即清理。`request_permissions`、MCP elicitation 和工具用户输入仍作为后续扩展请求 UI，不伪装为已经支持。
- 验证：本机 `codex-cli 0.146.0` 重新生成 v2 Schema 并核对请求/响应字段；`npm run typecheck` 通过；`npm test -- --run` 8 文件/11 测试通过；`cargo test --lib` 33 通过/1 忽略；strict Clippy 通过；`npm run build` 通过；Playwright 3 通过；`npm run tauri:build` 生成 release EXE 和 NSIS 成功。未运行会永久创建真实 Codex 会话的 ignored 测试。
- 未完成：App Server 通知流持久化、扩展客户端请求 UI、fake App Server 乱序/退出/超时/重复 ID 集成测试仍在 P0。
- Git 状态：`main` 跟踪 `origin/main`；本轮代码、测试和文档与此前未提交的总账/交接文档均为本地未提交修改，未推送；构建产物保持忽略。

### 2026-08-02 — 建立持续交接制度

- 目标：建立详细交接文档和所有后续对话共同维护的项目总账。
- 变化：新增 `AGENTS.md` 和 `docs/HANDOFF.md`，集中记录目标、架构、成果、性能事故、验证基线、P0/P1 队列和强制维护协议。
- 决策：使用工具可自动识别的根目录 `AGENTS.md`，而不是单数小写 `agent.md`。
- 验证：UTF-8 内容读取正常；所有引用的本地文件存在；敏感信息扫描无命中；`git diff --check` 通过。
- 未完成：代码功能未在本轮改变；文档尚未提交或推送。
- Git 状态：`main` 跟踪 `origin/main`；`AGENTS.md`、`docs/HANDOFF.md` 和 `docs/remaining-work.md` 是未提交本地修改。
