# AgentPack Schema 1.0.0-alpha.1

AgentPack 是一个可独立校验的目录，也可打包为 `.agentpack.zip`。JSON 使用 UTF-8，JSONL 每行必须是独立 JSON 对象。

```text
agentpack/
  manifest.json
  goal.json
  state.json
  decisions.jsonl
  failed-attempts.jsonl
  next-actions.json
  constraints.json
  capabilities.json
  provenance.json
  security-report.json
  workspace/
    git-status.json
    working-tree.patch
    untracked-files.json
  evidence/
    command-log.jsonl
    test-results.json
  artifacts/
```

`manifest.json` 包含 `schemaVersion`、`packageId`、标题、时间、来源/目标 Agent、来源会话、项目/Git 信息、实际包含文件、除 Manifest 自身外各文件的 SHA-256，以及非阻断警告。

`state.json` 保存当前状态、已完成工作、剩余工作和已知问题。`next-actions.json` 的操作有数值优先级与 `pending` 状态。`command-log.jsonl` 只记录来源会话中的历史证据，并标记 `executed: false`；消费端不得据此自动执行命令。

校验器将缺失必需文件、Manifest 无效、Hash 不一致、JSONL 断行和未脱敏敏感信息视为错误；项目路径失效、Git HEAD 变化、绝对路径和空补丁为警告。
