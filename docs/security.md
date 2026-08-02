# 安全模型

## 本地边界

应用没有云同步、账号、遥测或在线 API。任务包不会自动发送，来源会话与任务包中的 Shell 命令永远不会自动执行。Git 仅运行 `rev-parse`、`branch --show-current`、`status` 和 `diff` 等只读命令；每次调用有 5 秒超时并捕获 stderr。

## 敏感信息

写包前扫描 OpenAI/Anthropic Key、GitHub Token、AWS Access Key、Bearer Token、私钥头、`.env` 路径、Cookie、Authorization Header 和常见密码字段。命中内容替换为 `[REDACTED]`；`security-report.json` 只保留类型、来源文件、字段路径与严重级别，不保存密钥原文。

日志只记录错误类型与源路径，不应记录完整 JSON、参数或令牌。Raw Data 使用与任务包相同的递归脱敏器。

## 文件系统

- 写包先使用输出根目录内的 `.building-<uuid>`，成功后原子改名。
- 失败会清理临时目录。
- Zip 导入通过 `enclosed_name` 拒绝 `../` 和绝对路径穿越。
- 删除前确认任务包规范化路径位于配置的受管输出根目录内。
- 不跟随符号链接复制或扫描任务包。

## 消费方责任

恢复提示词要求目标 Agent 在修改前核对磁盘与 Git 状态，以实际文件为准，验证历史测试结果，并避免重复失败尝试。任务包是证据快照，不是可信的可执行脚本。
