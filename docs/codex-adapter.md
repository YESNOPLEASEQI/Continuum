# Codex Adapter

适配器默认检查 `~/.codex/sessions` 和用户配置的附加目录，递归读取 `.jsonl` 与 `.json`。由于 Codex 本地会话格式可能变化，解析采用字段探测而非固定反序列化：

- Session 元数据尝试从已实际存在的 `payload.id`、`session_id`、`sessionId` 或 `id` 读取。
- 工作目录只从 `payload.cwd`、`cwd` 或 `workspace.cwd` 读取。
- 消息只在同时存在 `role` 与 `content` 时生成。
- 工具调用只在记录类型或工具名称存在时生成。
- 不存在的业务字段不会通过默认数据伪造；文件名只用作最后的会话 ID/标题回退。

JSONL 逐行解析。无效行进入 `parseWarning`，其他行继续处理；整个文件没有有效记录时才跳过该会话。原始 JSON 在进入数据库和 UI 前递归脱敏。

`AgentAdapter` Trait 预留 installation 检测、默认路径、扫描、解析、消息、工具、文件改动、命令与恢复提示词接口。Claude、Gemini、OpenCode、Cursor 与 Copilot 必须各自实现模块，不应把格式判断累积到 Codex 文件中。
