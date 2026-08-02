# Roadmap

## Alpha 验收优先级

- 在安装 Rust/MSVC 的 Windows 环境完成 Tauri 原生编译与 `cargo test`。
- 使用已登录的 Codex CLI 完成完整桌面验收：启动新进程、检测真实 JSONL、绑定 session ID、增量同步、重启后验证关系。
- 将当前 2 秒 poll 升级为文件系统 watcher，并保留 poll 作为恢复机制。
- 增加启动超时、进程提前退出与上下文文件安全清理策略。
- 以更多真实 Codex 版本的匿名 fixture 扩展解析兼容测试。
- 增加 Continuation 历史与失败重试界面，以及多候选的更详细证据对比。

## Adapter 扩展

- Claude Code：先完成真实扫描、版本检测与能力确认，再开放自动启动。
- Gemini CLI、OpenCode：分别实现 Adapter，不把 Codex 路径或参数硬编码为通用能力。
- 本地模型压缩 Provider：保持 Context Item 的动作、原因与来源协议。
- 在线 Provider：必须由用户显式配置并清楚提示数据边界。

## 非主线

`.agentpack.zip`、任务包导入导出和签名机制只作为未来的统一会话导出能力评估，不再驱动产品信息架构。
