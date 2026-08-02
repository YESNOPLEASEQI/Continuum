import type { AgentKind } from "../types/models";

export interface AgentCapability {
  id: AgentKind;
  label: string;
  status: "available" | "planned";
  capabilities: string[];
  tools: string[];
  unsupported: string[];
}

export const agentCapabilities: AgentCapability[] = [
  {
    id: "codex",
    label: "Codex CLI",
    status: "available",
    capabilities: ["会话 JSONL", "Shell 工具记录", "文件改动", "Git 上下文", "任务包恢复提示"],
    tools: ["shell", "apply_patch", "git", "filesystem"],
    unsupported: [],
  },
  {
    id: "claude",
    label: "Claude Code",
    status: "planned",
    capabilities: ["项目上下文", "Shell 工具", "文件编辑", "子任务"],
    tools: ["shell", "filesystem", "git"],
    unsupported: ["Codex 专用工具调用元数据"],
  },
  {
    id: "gemini",
    label: "Gemini CLI",
    status: "planned",
    capabilities: ["项目上下文", "Shell 工具", "文件编辑"],
    tools: ["shell", "filesystem"],
    unsupported: ["Codex 会话分支", "部分工具结果结构"],
  },
  {
    id: "opencode",
    label: "OpenCode",
    status: "planned",
    capabilities: ["项目上下文", "Shell 工具", "文件编辑"],
    tools: ["shell", "filesystem", "git"],
    unsupported: ["Codex 专用工具调用元数据"],
  },
  {
    id: "cursor",
    label: "Cursor",
    status: "planned",
    capabilities: ["项目上下文", "文件编辑"],
    tools: ["filesystem"],
    unsupported: ["Shell 调用可移植性", "会话原始结构"],
  },
  {
    id: "copilot",
    label: "GitHub Copilot CLI",
    status: "planned",
    capabilities: ["Shell 建议", "GitHub 上下文"],
    tools: ["shell", "git"],
    unsupported: ["完整文件编辑历史", "Codex 工具结果"],
  },
];

export function getAgentLabel(agent: AgentKind): string {
  return agentCapabilities.find((item) => item.id === agent)?.label ?? agent;
}

export function calculateCompatibility(sourceId: AgentKind, targetId: AgentKind) {
  const source = agentCapabilities.find((item) => item.id === sourceId)!;
  const target = agentCapabilities.find((item) => item.id === targetId)!;
  const missingTools = source.tools.filter((tool) => !target.tools.includes(tool));
  const portable = source.capabilities.filter((capability) =>
    target.capabilities.some((candidate) => candidate.includes(capability.split(" ")[0])),
  );
  const base = 92 - missingTools.length * 12 - target.unsupported.length * 4;
  return {
    source,
    target,
    missingTools,
    portable,
    nonPortable: [...target.unsupported],
    score: Math.max(35, Math.min(100, sourceId === targetId ? 100 : base)),
  };
}
