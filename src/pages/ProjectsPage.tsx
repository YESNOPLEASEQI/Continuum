import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  Archive,
  ArrowRight,
  GitFork,
  FolderGit2,
  FolderKanban,
  FolderOpen,
  GitBranch,
  Link2,
  Pencil,
  Plus,
  RadioTower,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { appApi } from "../api/bridge";
import {
  Badge,
  ConfirmDialog,
  EmptyState,
  ErrorState,
  LoadingState,
  PageHeader,
  PathText,
} from "../components/ui";
import { getAgentLabel } from "../config/agents";
import { useAppStore } from "../store/appStore";
import type {
  ContextHealthLevel,
  CreateProjectInput,
  UnifiedProjectSummary,
} from "../types/models";

const initialInput: CreateProjectInput = {
  name: "",
  projectPath: "",
  goal: "",
  constraints: [],
  defaultAgent: "codex",
  defaultModel: "default",
};
const healthLabels: Record<ContextHealthLevel, string> = {
  healthy: "健康",
  growing: "开始冗长",
  compression_recommended: "建议压缩",
  fresh_continuation_recommended: "建议新会话",
  critical: "高风险",
};

export default function ProjectsPage() {
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const { projects, loading, error, loadProjects, notify } = useAppStore();
  const [creating, setCreating] = useState(params.get("create") === "1");
  const [input, setInput] = useState<CreateProjectInput>(initialInput);
  const [constraintText, setConstraintText] = useState("");
  const [saving, setSaving] = useState(false);
  const [archiving, setArchiving] = useState<UnifiedProjectSummary | null>(
    null,
  );
  const [deleting, setDeleting] = useState<UnifiedProjectSummary | null>(null);
  useEffect(() => {
    void loadProjects();
  }, [loadProjects]);
  useEffect(() => {
    if (params.get("create") === "1") setCreating(true);
  }, [params]);

  async function chooseProjectPath() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择统一项目工作目录",
    });
    if (selected && !Array.isArray(selected))
      setInput((value) => ({
        ...value,
        projectPath: selected,
        name:
          value.name || selected.split(/[\\/]/).filter(Boolean).at(-1) || "",
      }));
  }
  async function createProject() {
    setSaving(true);
    try {
      const project = await appApi.createProject({
        ...input,
        constraints: constraintText
          .split("\n")
          .map((line) => line.trim())
          .filter(Boolean),
      });
      notify({
        tone: "success",
        title: "统一项目已创建",
        detail: project.projectPath,
      });
      setCreating(false);
      setParams({});
      setInput(initialInput);
      await loadProjects();
      navigate(`/projects/${project.id}/chat`);
    } catch (reason) {
      notify({
        tone: "error",
        title: "创建失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    } finally {
      setSaving(false);
    }
  }
  async function archiveProject() {
    if (!archiving) return;
    await appApi.archiveProject(archiving.id);
    notify({ tone: "success", title: "项目已归档" });
    setArchiving(null);
    await loadProjects();
  }
  async function restoreProject(project: UnifiedProjectSummary) {
    await appApi.restoreProject(project.id);
    notify({ tone: "success", title: "项目已恢复", detail: project.name });
    await loadProjects();
  }
  async function renameProject(project: UnifiedProjectSummary) {
    const name = window.prompt("新的项目名称", project.name)?.trim();
    if (!name || name === project.name) return;
    await appApi.renameProject(project.id, name);
    notify({ tone: "success", title: "项目已重命名", detail: name });
    await loadProjects();
  }
  async function relocateProject(project: UnifiedProjectSummary) {
    const selected = await open({
      directory: true,
      multiple: false,
      title: `重新定位 ${project.name}`,
    });
    if (!selected || Array.isArray(selected)) return;
    await appApi.relocateProject(project.id, selected);
    notify({ tone: "success", title: "工作目录已重新定位", detail: selected });
    await loadProjects();
  }
  async function deleteProject() {
    if (!deleting) return;
    await appApi.deleteProjectRecord(deleting.id);
    notify({
      tone: "success",
      title: "Continuum 项目记录已删除",
      detail: "源码和 Codex 会话未被删除",
    });
    setDeleting(null);
    await loadProjects();
  }

  return (
    <div className="page projects-page">
      <PageHeader
        eyebrow="UNIFIED PROJECTS"
        title="统一项目"
        description="每个项目维护独立于 Agent 的主对话、分支图和上下文快照。"
        actions={
          <>
            <button
              className="button button-secondary"
              onClick={() => {
                setCreating(true);
                void chooseProjectPath();
              }}
            >
              <FolderOpen size={15} />
              导入已有目录
            </button>
            <button
              className="button button-primary"
              onClick={() => setCreating(true)}
            >
              <Plus size={15} />
              创建项目
            </button>
          </>
        }
      />
      {loading && !projects.length ? (
        <LoadingState label="正在读取统一项目" />
      ) : error ? (
        <ErrorState message={error} onRetry={() => void loadProjects()} />
      ) : !projects.filter((item) => !item.archived).length ? (
        <EmptyState
          icon={<FolderKanban size={24} />}
          title="还没有统一项目"
          detail="选择一个真实工作目录，将已有 Codex 会话绑定为同一条连续项目对话。"
          action={
            <button
              className="button button-primary"
              onClick={() => setCreating(true)}
            >
              创建第一个项目
            </button>
          }
        />
      ) : (
        <div className="project-list">
          {projects
            .filter((item) => !item.archived)
            .map((project) => (
              <article className="project-card" key={project.id}>
                <div className="project-mark">
                  <FolderGit2 size={20} />
                  <span>{project.sessionCount}</span>
                </div>
                <div className="project-main">
                  <div className="project-title-row">
                    <button
                      onClick={() => navigate(`/projects/${project.id}/chat`)}
                    >
                      <strong>{project.name}</strong>
                    </button>
                    <HealthBadge level={project.health.level} />
                    {!project.pathExists && (
                      <Badge tone="danger">
                        <AlertTriangle size={12} />
                        路径丢失
                      </Badge>
                    )}
                  </div>
                  <PathText value={project.projectPath} />
                  <p>{project.currentTask || project.goal}</p>
                  <div className="project-meta">
                    <span>
                      <GitBranch size={13} />
                      {project.currentBranchName}
                    </span>
                    <span>
                      <RadioTower size={13} />
                      {project.sessionCount} 个底层会话
                    </span>
                    <span>
                      {getAgentLabel(project.defaultAgent)} ·{" "}
                      {project.defaultModel}
                    </span>
                    <span>
                      {new Date(project.updatedAt).toLocaleString("zh-CN")}
                    </span>
                  </div>
                </div>
                <div className="project-actions">
                  {!project.pathExists && (
                    <button
                      className="button button-secondary"
                      onClick={() => void relocateProject(project)}
                    >
                      <FolderOpen size={14} />
                      重新定位
                    </button>
                  )}
                  <button
                    className="button button-secondary"
                    onClick={() => void renameProject(project)}
                  >
                    <Pencil size={14} />
                    重命名
                  </button>
                  <button
                    className="button button-secondary"
                    onClick={() => setArchiving(project)}
                  >
                    <Archive size={14} />
                    归档
                  </button>
                  <button
                    className="button button-secondary"
                    onClick={() => navigate(`/projects/${project.id}/context`)}
                  >
                    <GitFork size={14} />
                    检查上下文
                  </button>
                  <button
                    className="button button-primary"
                    onClick={() => navigate(`/projects/${project.id}/chat`)}
                    disabled={!project.pathExists}
                  >
                    打开对话 <ArrowRight size={14} />
                  </button>
                </div>
              </article>
            ))}
        </div>
      )}
      {projects.some((item) => item.archived) && (
        <section className="archived-projects">
          <h2>已归档项目</h2>
          {projects
            .filter((item) => item.archived)
            .map((project) => (
              <div className="archived-project-row" key={project.id}>
                <div>
                  <strong>{project.name}</strong>
                  <PathText value={project.projectPath} />
                </div>
                <button
                  className="button button-secondary"
                  onClick={() => void restoreProject(project)}
                >
                  <RotateCcw size={14} />
                  恢复
                </button>
                <button
                  className="button button-secondary danger"
                  onClick={() => setDeleting(project)}
                >
                  <Trash2 size={14} />
                  删除记录
                </button>
              </div>
            ))}
        </section>
      )}
      {creating && (
        <div
          className="dialog-backdrop"
          role="presentation"
          onMouseDown={() => setCreating(false)}
        >
          <form
            className="dialog project-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="create-project-title"
            onMouseDown={(event) => event.stopPropagation()}
            onSubmit={(event) => {
              event.preventDefault();
              void createProject();
            }}
          >
            <div className="dialog-mark continuum">
              <Link2 size={19} />
            </div>
            <h2 id="create-project-title">创建统一项目</h2>
            <p>
              项目状态属于 Continuum，不依赖某个 Agent。之后可绑定多个真实会话。
            </p>
            <label>
              <span>项目名称</span>
              <input
                value={input.name}
                onChange={(event) =>
                  setInput({ ...input, name: event.target.value })
                }
                required
              />
            </label>
            <label>
              <span>项目路径</span>
              <div className="inline-input">
                <input
                  value={input.projectPath}
                  onChange={(event) =>
                    setInput({ ...input, projectPath: event.target.value })
                  }
                  required
                />
                <button
                  type="button"
                  className="button button-secondary"
                  onClick={() => void chooseProjectPath()}
                >
                  选择
                </button>
              </div>
            </label>
            <label>
              <span>总体目标</span>
              <textarea
                rows={4}
                value={input.goal}
                onChange={(event) =>
                  setInput({ ...input, goal: event.target.value })
                }
                required
              />
            </label>
            <label>
              <span>长期约束（每行一项）</span>
              <textarea
                rows={3}
                value={constraintText}
                onChange={(event) => setConstraintText(event.target.value)}
              />
            </label>
            <div className="two-field">
              <label>
                <span>默认 Agent</span>
                <select
                  value={input.defaultAgent}
                  onChange={(event) =>
                    setInput({
                      ...input,
                      defaultAgent: event.target
                        .value as CreateProjectInput["defaultAgent"],
                    })
                  }
                >
                  <option value="codex">Codex CLI</option>
                </select>
              </label>
              <label>
                <span>默认模型</span>
                <input
                  value={input.defaultModel}
                  onChange={(event) =>
                    setInput({ ...input, defaultModel: event.target.value })
                  }
                />
              </label>
            </div>
            <div className="dialog-actions">
              <button
                type="button"
                className="button button-secondary"
                onClick={() => setCreating(false)}
              >
                取消
              </button>
              <button className="button button-primary" disabled={saving}>
                {saving ? "创建中" : "创建并打开"}
              </button>
            </div>
          </form>
        </div>
      )}
      <ConfirmDialog
        open={Boolean(archiving)}
        title="归档统一项目？"
        description="会话绑定和上下文快照会保留在本地数据库中，项目将从默认列表隐藏。"
        confirmLabel="归档"
        onCancel={() => setArchiving(null)}
        onConfirm={() => void archiveProject()}
      />
      <ConfirmDialog
        open={Boolean(deleting)}
        title="删除 Continuum 项目记录？"
        description="仅删除 Continuum 数据库中的项目、分支、绑定和快照；不会删除源码，也不会删除真实 Codex 会话。"
        confirmLabel="删除记录"
        destructive
        onCancel={() => setDeleting(null)}
        onConfirm={() => void deleteProject()}
      />
    </div>
  );
}

export function HealthBadge({ level }: { level: ContextHealthLevel }) {
  const tone =
    level === "healthy"
      ? "success"
      : level === "growing"
        ? "signal"
        : level === "compression_recommended"
          ? "warning"
          : "danger";
  return <Badge tone={tone}>{healthLabels[level]}</Badge>;
}
