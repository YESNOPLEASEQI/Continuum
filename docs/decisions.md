# 技术决策

## 2026-07-31：Fresh Continuation 是第一核心功能

默认续接不调用 `codex resume` 或 `codex fork`。上下文先写入项目内 Markdown，启动参数只携带短提示、绝对路径和唯一 `CONTINUATION_ID`。创建时间、规范化工作目录、marker、Agent 类型与未绑定状态共同决定候选；“最新会话文件”不构成充分证据。

创建快照/写文件和启动进程拆成两个后端命令，让 UI 的 `writing_context` 与 `launching` 对应真实完成边界。绑定后由 Continuation 记录持有 session ID、PID、Hash、时间与 listening 状态，并在统一时间线插入压缩/切换事件。

## 2026-07-31：产品主轴改为统一会话与上下文续接

原型最初以 `Session → AgentPack` 为核心。现在改为 `Project → Branch → ConversationNode → ContextSnapshot → Continuation`。任务包与 Zip 模块保留为 `legacy/export`，从主导航移除。SQLite、Codex Adapter、Git Inspector、安全扫描和原子文件写入继续复用。

统一时间线不是抹平来源的消息数组：每个节点必须保留 `sourceAgent`、`sourceSessionId`、`parentNodeId` 与 `branchId`。Codex 第一版只声明“上下文续接”，不会宣称可写入或恢复原生内部会话。

## SQLite 使用 bundled rusqlite

选择 `rusqlite` 的 bundled SQLite，减少 Windows 上额外 DLL 与版本差异。数据库采用 WAL、外键和 5 秒 busy timeout；首次启动执行幂等迁移。

## Hash 不包含 Manifest

Manifest 保存其他文件的 SHA-256，避免自引用哈希。所有内容文件先完成脱敏与落盘，再计算哈希，最后写 Manifest。

## Codex 使用容错 Value 解析

本地会话格式不是稳定公共契约。使用 `serde_json::Value` 探测真实字段，使未知记录可保留、坏行可跳过，也避免编造数据。前端 Raw Data 只接收已脱敏副本。

## Git 使用受限子进程

Git CLI 在目标环境普遍存在，且能准确反映工作树。所有命令在固定参数表中构造、禁用 stdin、捕获 stderr、设置超时，不拼接 Shell 字符串。

## 浏览器预览不使用 Mock 会话

Vite 浏览器模式只提供零值 Dashboard 和“需要桌面运行时”错误，便于视觉开发但不伪造业务数据。正式 Tauri 模式完全使用 SQLite 和文件系统。

## 视觉语言

界面采用石墨黑、冷蓝信号色与琥珀告警色，以边线和密度而非圆角卡片组织信息。侧栏“接力轨迹”是唯一显著视觉签名，对应扫描、封装、恢复三个阶段。
