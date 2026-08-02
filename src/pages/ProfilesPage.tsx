import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Check,
  Copy,
  Download,
  FolderOpen,
  Plus,
  RefreshCw,
  Save,
  ShieldCheck,
  TerminalSquare,
  Trash2,
  Upload,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { appApi } from "../api/bridge";
import {
  Badge,
  ConfirmDialog,
  EmptyState,
  ErrorState,
  LoadingState,
  PageHeader,
  Toggle,
} from "../components/ui";
import { useAppStore } from "../store/appStore";
import type { CodexCapabilityReport, CodexProfile } from "../types/models";

export default function ProfilesPage() {
  const { projects, loadProjects, notify } = useAppStore();
  const [projectId, setProjectId] = useState("");
  const [profiles, setProfiles] = useState<CodexProfile[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<CodexProfile | null>(null);
  const [capabilities, setCapabilities] =
    useState<CodexCapabilityReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);

  const selectedProject = useMemo(
    () => projects.find((project) => project.id === projectId) ?? null,
    [projectId, projects],
  );

  async function load(preferredId?: string) {
    setLoading(true);
    try {
      const [items, report] = await Promise.all([
        appApi.codexProfiles(projectId || undefined),
        appApi.detectCodex(false),
      ]);
      setProfiles(items);
      setCapabilities(report);
      const nextId = preferredId ?? selectedId ?? items[0]?.id ?? null;
      const next = items.find((item) => item.id === nextId) ?? items[0] ?? null;
      setSelectedId(next?.id ?? null);
      setDraft(next ? structuredClone(next) : null);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadProjects();
  }, [loadProjects]);

  useEffect(() => {
    setSelectedId(null);
    void load();
  }, [projectId]);

  function select(profile: CodexProfile) {
    setSelectedId(profile.id);
    setDraft(structuredClone(profile));
  }

  async function createProfile() {
    try {
      const profile = await appApi.createCodexProfile(
        projectId || undefined,
        selectedProject?.currentBranchId,
      );
      await load(profile.id);
      notify({ tone: "success", title: "Codex Profile 已创建" });
    } catch (reason) {
      notify({
        tone: "error",
        title: "创建失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    }
  }

  async function saveProfile() {
    if (!draft) return;
    setSaving(true);
    try {
      const saved = await appApi.saveCodexProfile(draft);
      await load(saved.id);
      notify({ tone: "success", title: "Profile 已验证并保存" });
    } catch (reason) {
      notify({
        tone: "error",
        title: "Profile 未保存",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    } finally {
      setSaving(false);
    }
  }

  async function duplicateProfile() {
    if (!draft) return;
    try {
      const copy = await appApi.duplicateCodexProfile(
        draft.id,
        `${draft.name} Copy`,
      );
      await load(copy.id);
      notify({ tone: "success", title: "Profile 已复制" });
    } catch (reason) {
      notify({
        tone: "error",
        title: "复制失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    }
  }

  async function removeProfile() {
    if (!draft) return;
    try {
      await appApi.deleteCodexProfile(draft.id);
      setDeleteOpen(false);
      setSelectedId(null);
      await load();
      notify({ tone: "success", title: "Profile 记录已删除" });
    } catch (reason) {
      notify({
        tone: "error",
        title: "删除失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    }
  }

  async function chooseExecutable() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "选择 Codex 可执行文件",
    });
    if (selected && !Array.isArray(selected))
      setDraft((value) =>
        value ? { ...value, executablePath: selected } : value,
      );
  }

  async function chooseWorkingDirectory() {
    const selected = await open({
      multiple: false,
      directory: true,
      title: "选择 Profile 工作目录",
    });
    if (selected && !Array.isArray(selected))
      setDraft((value) =>
        value ? { ...value, workingDirectory: selected } : value,
      );
  }

  async function importProfile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Continuum Codex Profile", extensions: ["json"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    try {
      const imported = await appApi.importCodexProfile(selected);
      await load(imported.id);
      notify({ tone: "success", title: "Profile 已导入并验证" });
    } catch (reason) {
      notify({
        tone: "error",
        title: "导入失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    }
  }

  async function exportProfile() {
    if (!draft) return;
    const destination = await save({
      defaultPath: `${draft.name.replace(/[\\/:*?"<>|]/g, "-")}.continuum-profile.json`,
      filters: [{ name: "Continuum Codex Profile", extensions: ["json"] }],
    });
    if (!destination) return;
    try {
      const path = await appApi.exportCodexProfile(draft.id, destination);
      notify({ tone: "success", title: "Profile 已导出", detail: path });
    } catch (reason) {
      notify({
        tone: "error",
        title: "导出失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    }
  }

  async function setDefault(scope: "project" | "branch") {
    if (!draft || !selectedProject) return;
    try {
      if (scope === "project")
        await appApi.setProjectCodexProfile(selectedProject.id, draft.id);
      else
        await appApi.setBranchCodexProfile(
          selectedProject.id,
          selectedProject.currentBranchId,
          draft.id,
        );
      notify({
        tone: "success",
        title:
          scope === "project"
            ? "已设为项目默认 Profile"
            : "已设为当前分支默认 Profile",
      });
    } catch (reason) {
      notify({
        tone: "error",
        title: "设置默认 Profile 失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    }
  }

  return (
    <div className="page profiles-page">
      <PageHeader
        eyebrow="CODEX EXECUTION PROFILES"
        title="Codex Profiles"
        description="集中管理 Codex 可执行文件、模型、安全策略、上下文预算和 Fresh Continuation 启动模板。保存前会使用本机 Codex 能力进行严格校验。"
        actions={
          <>
            <label className="project-filter">
              <span>项目范围</span>
              <select
                value={projectId}
                onChange={(event) => setProjectId(event.target.value)}
              >
                <option value="">全局 Profiles</option>
                {projects
                  .filter((project) => !project.archived)
                  .map((project) => (
                    <option key={project.id} value={project.id}>
                      {project.name}
                    </option>
                  ))}
              </select>
            </label>
            <button
              className="button button-secondary"
              onClick={() => void importProfile()}
            >
              <Upload size={14} />
              导入
            </button>
            <button
              className="button button-primary"
              onClick={() => void createProfile()}
            >
              <Plus size={14} />
              新建 Profile
            </button>
          </>
        }
      />

      {capabilities && (
        <div
          className={`capability-strip ${capabilities.installed ? "ready" : "error"}`}
        >
          <ShieldCheck size={16} />
          <strong>
            {capabilities.installed ? capabilities.version : "未检测到 Codex"}
          </strong>
          <code>{capabilities.executablePath ?? capabilities.error}</code>
          <span>Resume {capabilities.supportsResume ? "✓" : "—"}</span>
          <span>Fork {capabilities.supportsFork ? "✓" : "—"}</span>
          <span>Sandbox {capabilities.supportsSandbox ? "✓" : "—"}</span>
          <span>App Server {capabilities.supportsAppServer ? "✓" : "—"}</span>
          <button
            className="icon-button"
            title="重新检测 Codex"
            onClick={async () => {
              setCapabilities(await appApi.detectCodex(true));
            }}
          >
            <RefreshCw size={14} />
          </button>
        </div>
      )}

      {loading && !draft ? (
        <LoadingState label="正在读取 Codex Profiles" />
      ) : error ? (
        <ErrorState message={error} onRetry={() => void load()} />
      ) : !draft ? (
        <EmptyState
          icon={<TerminalSquare size={22} />}
          title="尚无 Codex Profile"
          detail="创建第一个 Profile 后，Continuum 会先验证可执行文件、能力和安全参数。"
          action={
            <button
              className="button button-primary"
              onClick={() => void createProfile()}
            >
              <Plus size={14} />
              新建 Profile
            </button>
          }
        />
      ) : (
        <div className="profile-layout">
          <aside className="profile-list" aria-label="Codex Profiles">
            {profiles.map((profile) => (
              <button
                key={profile.id}
                className={selectedId === profile.id ? "active" : ""}
                onClick={() => select(profile)}
              >
                <TerminalSquare size={15} />
                <span>
                  <strong>{profile.name}</strong>
                  <small>
                    {profile.model || "默认模型"} ·{" "}
                    {profile.contextBudget.toLocaleString()} tokens
                  </small>
                </span>
                {profile.projectId ? (
                  <Badge tone="signal">项目</Badge>
                ) : (
                  <Badge>全局</Badge>
                )}
              </button>
            ))}
          </aside>

          <section className="profile-editor">
            <header>
              <div>
                <p className="eyebrow">PROFILE EDITOR</p>
                <h2>{draft.name}</h2>
              </div>
              <div>
                <button
                  className="button button-secondary"
                  onClick={() => void duplicateProfile()}
                >
                  <Copy size={13} />
                  复制
                </button>
                <button
                  className="button button-secondary"
                  onClick={() => void exportProfile()}
                >
                  <Download size={13} />
                  导出
                </button>
                <button
                  className="icon-button danger"
                  title="删除 Profile"
                  onClick={() => setDeleteOpen(true)}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </header>

            <div className="profile-form">
              <label>
                <span>Profile 名称</span>
                <input
                  value={draft.name}
                  onChange={(event) =>
                    setDraft({ ...draft, name: event.target.value })
                  }
                />
              </label>
              <label>
                <span>模型（留空使用 Codex 默认值）</span>
                <input
                  value={draft.model ?? ""}
                  onChange={(event) =>
                    setDraft({ ...draft, model: event.target.value || null })
                  }
                  placeholder="例如 gpt-5.6-codex"
                />
              </label>
              <label className="wide">
                <span>Codex 可执行文件</span>
                <div className="inline-input">
                  <input
                    value={draft.executablePath}
                    onChange={(event) =>
                      setDraft({ ...draft, executablePath: event.target.value })
                    }
                  />
                  <button
                    className="button button-secondary"
                    onClick={() => void chooseExecutable()}
                  >
                    <FolderOpen size={13} />
                    选择
                  </button>
                </div>
              </label>
              <label className="wide">
                <span>显式工作目录</span>
                <div className="inline-input">
                  <input
                    value={draft.workingDirectory}
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        workingDirectory: event.target.value,
                      })
                    }
                  />
                  <button
                    className="button button-secondary"
                    onClick={() => void chooseWorkingDirectory()}
                  >
                    <FolderOpen size={13} />
                    选择
                  </button>
                </div>
              </label>
              <label>
                <span>Approval Mode</span>
                <select
                  value={draft.approvalMode}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      approvalMode: event.target
                        .value as CodexProfile["approvalMode"],
                    })
                  }
                >
                  <option value="untrusted">untrusted</option>
                  <option value="on-request">on-request</option>
                  <option value="never">never</option>
                </select>
              </label>
              <label>
                <span>Sandbox Mode</span>
                <select
                  value={draft.sandboxMode}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      sandboxMode: event.target
                        .value as CodexProfile["sandboxMode"],
                    })
                  }
                >
                  <option value="read-only">read-only</option>
                  <option value="workspace-write">workspace-write</option>
                  <option value="danger-full-access">danger-full-access</option>
                </select>
              </label>
              <label>
                <span>上下文预算</span>
                <input
                  type="number"
                  min={1000}
                  max={1000000}
                  value={draft.contextBudget}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      contextBudget: Number(event.target.value),
                    })
                  }
                />
              </label>
              <label>
                <span>保留最近消息</span>
                <input
                  type="number"
                  min={1}
                  max={500}
                  value={draft.recentMessageLimit}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      recentMessageLimit: Number(event.target.value),
                    })
                  }
                />
              </label>
              <label className="wide">
                <span>附加启动参数（每行一个，严格白名单）</span>
                <textarea
                  value={draft.launchArguments.join("\n")}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      launchArguments: event.target.value
                        .split(/\r?\n/)
                        .map((value) => value.trim())
                        .filter(Boolean),
                    })
                  }
                  placeholder="--no-alt-screen"
                />
              </label>
              <label className="wide">
                <span>Fresh Continuation 启动模板</span>
                <textarea
                  className="prompt-template"
                  value={draft.launchPromptTemplate}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      launchPromptTemplate: event.target.value,
                    })
                  }
                />
                <small>
                  必须保留 {"{{CONTEXT_FILE_PATH}}"} 和{" "}
                  {"{{CONTINUATION_MARKER}}"}。
                </small>
              </label>
            </div>

            <div className="profile-toggles">
              <Toggle
                checked={draft.includeGitStatus}
                onChange={(checked) =>
                  setDraft({ ...draft, includeGitStatus: checked })
                }
                label="Git 状态"
                detail="启动前核对工作区状态"
              />
              <Toggle
                checked={draft.includeGitDiff}
                onChange={(checked) =>
                  setDraft({ ...draft, includeGitDiff: checked })
                }
                label="Git Diff"
                detail="允许加入压缩后的差异摘要"
              />
              <Toggle
                checked={draft.includeTests}
                onChange={(checked) =>
                  setDraft({ ...draft, includeTests: checked })
                }
                label="测试状态"
                detail="编译测试结果与失败信息"
              />
              <Toggle
                checked={draft.includeFailedAttempts}
                onChange={(checked) =>
                  setDraft({ ...draft, includeFailedAttempts: checked })
                }
                label="失败尝试"
                detail="保留仍有决策价值的失败路径"
              />
              <Toggle
                checked={draft.includeSkills}
                onChange={(checked) =>
                  setDraft({ ...draft, includeSkills: checked })
                }
                label="Skills"
                detail="带入已绑定 Skill 清单"
              />
              <Toggle
                checked={draft.includeMcp}
                onChange={(checked) =>
                  setDraft({ ...draft, includeMcp: checked })
                }
                label="MCP"
                detail="带入已绑定 MCP Server 清单"
              />
            </div>

            <footer>
              <div>
                {selectedProject && (
                  <button
                    className="button button-secondary"
                    onClick={() => void setDefault("project")}
                  >
                    <Check size={13} />
                    设为项目默认
                  </button>
                )}
                {selectedProject && (
                  <button
                    className="button button-secondary"
                    onClick={() => void setDefault("branch")}
                  >
                    <Check size={13} />
                    设为当前分支默认
                  </button>
                )}
              </div>
              <button
                className="button button-primary"
                onClick={() => void saveProfile()}
                disabled={saving}
              >
                <Save size={14} />
                {saving ? "验证中" : "验证并保存"}
              </button>
            </footer>
          </section>
        </div>
      )}

      <ConfirmDialog
        open={deleteOpen}
        title="删除 Codex Profile？"
        description="只删除 Continuum 中的 Profile 记录，不会删除 Codex、项目文件或会话。使用它的默认绑定会被解除。"
        confirmLabel="删除 Profile"
        destructive
        onConfirm={() => void removeProfile()}
        onCancel={() => setDeleteOpen(false)}
      />
    </div>
  );
}
