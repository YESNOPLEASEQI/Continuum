import { z } from "zod";

export const agentKindSchema = z.enum(["codex", "claude", "gemini", "opencode", "cursor", "copilot"]);

export const packageDraftSchema = z.object({
  sourceSessionId: z.string().min(1, "请选择来源会话"),
  title: z.string().trim().min(2, "标题至少需要 2 个字符").max(160, "标题不能超过 160 个字符"),
  originalGoal: z.string().trim().min(1, "请填写原始目标"),
  currentState: z.string().trim().min(1, "请填写当前状态"),
  completedWork: z.string(),
  remainingWork: z.string(),
  nextActions: z.string().trim().min(1, "至少需要一项下一步操作"),
  decisions: z.string(),
  knownIssues: z.string(),
  failedAttempts: z.string(),
  constraints: z.string(),
  requiredTools: z.string(),
  targetAgent: agentKindSchema,
  includeGit: z.boolean(),
  includePatch: z.boolean(),
  includeUntracked: z.boolean(),
  includeTests: z.boolean(),
  includeCommandLog: z.boolean(),
});

export const manifestSchema = z.object({
  schemaVersion: z.literal("1.0.0-alpha.1"),
  packageId: z.string().min(1),
  title: z.string().min(1),
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
  sourceAgent: agentKindSchema,
  targetAgent: agentKindSchema,
  sourceSessionId: z.string().min(1),
  projectPath: z.string().nullable(),
  gitRepository: z.string().nullable(),
  gitHead: z.string().nullable(),
  includedSections: z.array(z.string()),
  contentHashes: z.record(z.string(), z.string()),
  warnings: z.array(z.string()),
});

export type PackageDraftInput = z.infer<typeof packageDraftSchema>;
