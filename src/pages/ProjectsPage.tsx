import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  Archive,
  ArrowRight,
  ArrowUpRight,
  Clock3,
  Database,
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
  X,
} from "lucide-react";
import { useGSAP } from "@gsap/react";
import gsap from "gsap";
import { useEffect, useMemo, useRef, useState } from "react";
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
import { ProjectArchiveDeck } from "../components/ProjectArchiveDeck";
import { getAgentLabel } from "../config/agents";
import { useAppStore } from "../store/appStore";
import { useContinuumMotion } from "../motion/ContinuumMotion";
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
  const pageRef = useRef<HTMLDivElement>(null);
  const overviewTimeline = useRef<gsap.core.Timeline | null>(null);
  const { navigateMajor } = useContinuumMotion();
  const [params, setParams] = useSearchParams();
  const { projects, sessions, loading, error, loadProjects, loadSessions, notify } = useAppStore();
  const [creating, setCreating] = useState(params.get("create") === "1");
  const [input, setInput] = useState<CreateProjectInput>(initialInput);
  const [constraintText, setConstraintText] = useState("");
  const [saving, setSaving] = useState(false);
  const [archiving, setArchiving] = useState<UnifiedProjectSummary | null>(
    null,
  );
  const [deleting, setDeleting] = useState<UnifiedProjectSummary | null>(null);
  const [overview, setOverview] = useState<UnifiedProjectSummary | null>(null);
  const activeProjects = useMemo(
    () => projects.filter((item) => !item.archived),
    [projects],
  );
  useEffect(() => {
    void Promise.all([loadProjects(), loadSessions()]);
  }, [loadProjects, loadSessions]);
  useEffect(() => {
    if (params.get("create") === "1") setCreating(true);
  }, [params]);

  useGSAP(
    () => {
      const reduced = window.matchMedia?.(
        "(prefers-reduced-motion: reduce)",
      )?.matches;
      gsap.set(".project-overview-panel", {
        yPercent: reduced ? 0 : 112,
        rotationX: reduced ? 0 : -7,
        transformOrigin: "50% 100%",
        autoAlpha: 0,
      });
      overviewTimeline.current = gsap
        .timeline({ paused: true, defaults: { overwrite: "auto" } })
        .set(".project-overview-layer", { pointerEvents: "auto" })
        .to(
          ".project-overview-scrim",
          { autoAlpha: 1, duration: reduced ? 0 : 0.28, ease: "power2.out" },
          0,
        )
        .to(
          ".project-overview-panel",
          {
            yPercent: 0,
            rotationX: 0,
            autoAlpha: 1,
            duration: reduced ? 0 : 0.58,
            ease: "power3.out",
          },
          0,
        );
      overviewTimeline.current.eventCallback("onReverseComplete", () => {
        gsap.set(".project-overview-layer", { pointerEvents: "none" });
        setOverview(null);
      });
    },
    { scope: pageRef },
  );

  function openOverview(project: UnifiedProjectSummary) {
    const alreadyOpen = Boolean(overview);
    setOverview(project);
    if (!alreadyOpen) {
      requestAnimationFrame(() => overviewTimeline.current?.play(0));
    }
  }

  function closeOverview() {
    overviewTimeline.current?.reverse();
  }

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
    <div ref={pageRef} className="page projects-page continuum-home">
      <ProjectArchiveDeck
        projects={projects}
        sessions={sessions}
        loading={loading}
        error={error}
        onRetry={() => void Promise.all([loadProjects(), loadSessions()])}
        onOpenProject={openOverview}
        onOpenSession={(session) => navigate(`/sessions/${session.id}`)}
        onBrowseSessions={() => navigate("/sessions")}
        onCreateProject={() => setCreating(true)}
        onImportProject={() => {
          setCreating(true);
          void chooseProjectPath();
        }}
      />
      <section className="continuum-entry" aria-labelledby="continuum-entry-title">
        <div className="entry-meta entry-reveal">
          <span>本地 Codex 会话编排</span>
          <span>SQLite / App Server / CLI fallback</span>
        </div>
        <div className="entry-wordmark" id="continuum-entry-title" role="heading" aria-level={1} aria-label="Continuum">
          {Array.from("CONTINUUM").map((letter, index) => (
            <span className="entry-letter" aria-hidden="true" key={`${letter}-${index}`}>
              {letter}
            </span>
          ))}
        </div>
        <div className="entry-footer entry-reveal">
          <div>
            <p>把分散的会话，组织成一条可继续的工作脉络。</p>
            <small>上下文留在本机。Fresh 不继承旧会话历史。</small>
          </div>
          <div className="entry-actions">
            <button
              className="entry-link"
              onClick={() => navigate("/sessions")}
            >
              浏览来源会话 <ArrowUpRight size={14} />
            </button>
            <button
              className="entry-primary"
              disabled={!activeProjects[0]?.pathExists}
              onClick={() => activeProjects[0] && openOverview(activeProjects[0])}
            >
              进入最近项目 <ArrowRight size={14} />
            </button>
          </div>
        </div>
      </section>
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
      ) : !activeProjects.length ? (
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
        <div className="project-index" aria-label="统一项目索引">
          <div className="project-index-head entry-reveal">
            <span>INDEX</span><span>PROJECT / CURRENT THREAD</span><span>UPDATED</span><span>STATE</span>
          </div>
          {activeProjects.map((project, projectIndex) => (
              <article className="project-card project-index-row entry-reveal" key={project.id}>
                <div className="project-mark">
                  <span>{String(projectIndex + 1).padStart(2, "0")}</span>
                  <FolderGit2 size={17} />
                </div>
                <div className="project-main">
                  <div className="project-title-row">
                    <button
                      onClick={() => openOverview(project)}
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
                <div className="project-updated">
                  <Clock3 size={13} />
                  <time>{new Date(project.updatedAt).toLocaleDateString("zh-CN", { month: "2-digit", day: "2-digit" })}</time>
                </div>
                <div className="project-actions project-row-actions">
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
                    className="project-open"
                    onClick={() => openOverview(project)}
                    disabled={!project.pathExists}
                    aria-label={`查看 ${project.name} 概览`}
                  >
                    <ArrowUpRight size={17} />
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
      <div className="project-overview-layer" aria-hidden={!overview}>
        <button
          className="project-overview-scrim"
          aria-label="关闭项目概览"
          onClick={closeOverview}
        />
        <section className="project-overview-panel" role="dialog" aria-modal="true" aria-label={overview ? `${overview.name} 项目概览` : "项目概览"}>
          {overview && (
            <>
              <header>
                <div>
                  <span>PROJECT OVERVIEW / {overview.currentBranchName}</span>
                  <h2>{overview.name}</h2>
                </div>
                <button aria-label="关闭项目概览" onClick={closeOverview}><X size={18} /></button>
              </header>
              <div className="project-overview-body">
                <div className="overview-thesis">
                  <p>{overview.currentTask || overview.goal}</p>
                  <PathText value={overview.projectPath} />
                </div>
                <dl>
                  <div><dt>当前分支</dt><dd>{overview.currentBranchName}</dd></div>
                  <div><dt>来源会话</dt><dd>{overview.sessionCount}</dd></div>
                  <div><dt>Context budget</dt><dd>{Math.round(overview.health.thresholdRatio * 100)}%</dd></div>
                  <div><dt>本地状态</dt><dd>{overview.pathExists ? "READY" : "PATH MISSING"}</dd></div>
                </dl>
                <div className="overview-health-line">
                  <HealthBadge level={overview.health.level} />
                  <span>{overview.health.reasons[0] || "上下文状态稳定"}</span>
                </div>
              </div>
              <footer>
                <div className="overview-quiet-actions">
                  <button onClick={() => void renameProject(overview)}><Pencil size={13} />重命名</button>
                  <button onClick={() => setArchiving(overview)}><Archive size={13} />归档</button>
                  <button onClick={() => navigate(`/projects/${overview.id}/context`)}><GitFork size={13} />Context</button>
                </div>
                <button
                  className="overview-enter"
                  disabled={!overview.pathExists}
                  onClick={() => void navigateMajor(`/projects/${overview.id}/chat`, `进入 ${overview.name}`)}
                >
                  打开项目工作区 <ArrowRight size={15} />
                </button>
              </footer>
              <div className="overview-watermark" aria-hidden="true"><Database size={18} />CONTINUUM / LOCAL</div>
            </>
          )}
        </section>
      </div>
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
