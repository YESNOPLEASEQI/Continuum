# Continuum 产品定义

## 产品定位

Continuum 是一个跨 Agent 的统一会话与上下文续接客户端。它把 Codex、Claude Code、Gemini CLI、OpenCode 等工具产生的分散会话，组织为独立于任何单一 Agent 的连续项目对话。

产品核心不是把原始消息无限拼接，也不是管理离线任务包，而是维护三层状态：

1. `Unified Project`：项目目标、长期约束、工作区、默认 Agent/模型与绑定配置。
2. `Conversation Graph`：分支、父节点、来源 Agent、来源会话与事件类型。
3. `Context Snapshot`：每次续接时，按预算编译并可解释地保留、压缩、检索或排除的上下文。

## 真实闭环

第一版优先完成：

```text
扫描真实 Codex 会话
  -> 创建统一项目
  -> 绑定一个或多个来源会话
  -> 按来源和时间合并为连续时间线
  -> 从任意节点创建分支
  -> 规则式编译续接上下文
  -> 保存 Context Snapshot
  -> 写入项目内临时 Markdown 上下文文件
  -> 在显式工作目录启动全新 Codex（不使用 resume/fork）
  -> 用唯一 CONTINUATION_ID 自动识别并绑定新 session ID
  -> 轮询同步新增消息
```

## 三种操作必须分离

- `Resume`：恢复原会话，保留长历史。
- `Fork`：从原历史分叉，仍可能继承旧历史。
- `Fresh Continuation`：压缩后启动干净会话，仅注入必要上下文；这是主功能。

Fresh Continuation 的新会话识别同时校验创建时间、工作目录、首条用户消息 marker、Codex Agent 类型和未绑定状态。不能仅选“最新文件”。多候选时必须让用户确认。

Continuum 不会把“逻辑上下文续接”描述为“原生会话迁移”。

## Context Compiler

编译器按四层处理上下文：

- 永久：项目目标、明确约束、关键决策、技术栈、工作目录与核心文件。
- 阶段：当前任务、最近改动、未解决问题、测试与 Git 状态。
- 短期：最近 N 轮消息、最近工具调用与错误。
- 检索：旧讨论、已完成过程、长输出和重复内容，不直接注入但保留引用。

每个 Context Item 保存 `keep | compress | retrieve | exclude` 处理动作、原因、来源节点和估算 Token。默认实现为确定性 `RuleBasedProvider`；未来的本地与在线 Provider 必须保持同一可解释输出协议。

## 非目标与降级能力

`.agentpack.zip`、复杂任务包导入导出、证据目录与 Package Detail 已降级为 `legacy/export`，不在主导航与核心流程中。底层哈希、Zip、安全扫描代码保留，未来可用于“导出统一会话”，但本轮不继续扩展。

## 数据与安全

SQLite 保存项目图、来源会话、上下文快照、续接记录和配置绑定。会话与上下文默认不离开本机。Continuum 不自动运行历史命令、不改写第三方会话文件、不自动安装 Skills/MCP，也不声称精确测量模型能力下降；Context Health 只是基于长度、重复、工具日志和陈旧信息的风险估算。
