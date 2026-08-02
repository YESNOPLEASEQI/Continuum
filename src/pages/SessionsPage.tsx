import {
  Clipboard,
  CheckCircle2,
  FolderKanban,
  Link2,
  MoreHorizontal,
  Play,
  RadioTower,
  RefreshCw,
  Search,
  Sparkles,
  Split,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { appApi } from "../api/bridge";
import {
  Badge,
  EmptyState,
  ErrorState,
  LoadingState,
  PageHeader,
} from "../components/ui";
import { useAppStore } from "../store/appStore";
import { useMajorSessionScan } from "../motion/useMajorSessionScan";
import type { SessionSummary } from "../types/models";

export default function SessionsPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const {
    sessions,
    projects,
    loading,
    scanning,
    error,
    loadSessions,
    loadProjects,
    notify,
  } = useAppStore();
  const scanSessions = useMajorSessionScan();
  const [query, setQuery] = useState("");
  const [visibleCount, setVisibleCount] = useState(30);
  const [projectId, setProjectId] = useState(params.get("project") ?? "");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  useEffect(() => {
    void Promise.all([loadSessions(), loadProjects()]);
  }, [loadSessions, loadProjects]);
  const visible = useMemo(
    () =>
      sessions
        .filter((item) =>
          `${item.title} ${item.boundProjectName ?? ""} ${item.workingDirectory ?? ""}`
            .toLowerCase()
            .includes(query.toLowerCase()),
        )
        .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)),
    [sessions, query],
  );
  useEffect(() => setVisibleCount(30), [query]);
  const renderedSessions = visible.slice(0, visibleCount);
  const workspaceName = (path: string | null) =>
    path?.split(/[\\/]/).filter(Boolean).at(-1) ?? "未记录项目";
  const clientLabel = (session: SessionSummary) =>
    session.clientKind === "desktop"
      ? "Codex Desktop"
      : session.clientKind === "cli"
        ? "Codex CLI"
        : "Codex";
  async function bind(ids: string[]) {
    if (!projectId) {
      notify({ tone: "info", title: "请选择统一项目" });
      return null;
    }
    try {
      const project = await appApi.bindSessions(projectId, ids);
      notify({
        tone: "success",
        title: "会话已绑定到统一项目",
        detail: `${ids.length} 个来源会话`,
      });
      return project;
    } catch (reason) {
      notify({
        tone: "error",
        title: "绑定失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
      return null;
    }
  }
  async function fresh(
    session: SessionSummary,
    target: "codex" | "claude" = "codex",
  ) {
    if (!projectId && session.boundProjectId) {
      const project = projects.find((item) => item.id === session.boundProjectId);
      if (project) {
        navigate(
          `/projects/${project.id}/continuation?branch=${project.currentBranchId}&target=${target}`,
        );
        return;
      }
    }
    const project = await bind([session.id]);
    if (project)
      navigate(
        `/projects/${project.id}/continuation?branch=${project.currentBranchId}&target=${target}`,
      );
  }
  async function launchSource(
    session: SessionSummary,
    operation: "resume" | "fork",
  ) {
    try {
      const pid = await appApi.launchSourceSession(session.id, operation);
      notify({
        tone: "success",
        title: operation === "resume" ? "已启动 Resume" : "已启动 Fork",
        detail: `PID ${pid}；此操作不会压缩旧历史`,
      });
    } catch (reason) {
      notify({
        tone: "error",
        title: "Codex 启动失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    }
  }
  function toggle(id: string) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  return (
    <div className="page sessions-page">
      <PageHeader
        eyebrow="SOURCE SESSIONS"
        title="原始 Agent 会话"
        description="扫描真实 Codex 会话；选择多个来源可合并到同一统一项目和连续时间线。"
        actions={
          <button
            className="button button-primary"
            onClick={() => void scanSessions()}
            disabled={scanning}
          >
            <RefreshCw size={14} className={scanning ? "animate-spin" : ""} />
            {scanning ? "扫描中" : "扫描 Codex"}
          </button>
        }
      />
      <div className="session-bindbar">
        <label className="search-field">
          <Search size={14} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索会话标题或项目"
          />
        </label>
        <label>
          <span>绑定到</span>
          <select
            value={projectId}
            onChange={(event) => setProjectId(event.target.value)}
          >
            <option value="">选择统一项目</option>
            {projects
              .filter((project) => !project.archived)
              .map((project) => (
                <option value={project.id} key={project.id}>
                  {project.name}
                </option>
              ))}
          </select>
        </label>
        <button
          className="button button-secondary"
          disabled={!selected.size || !projectId}
          onClick={async () => {
            const result = await bind([...selected]);
            if (result) {
              setSelected(new Set());
              navigate(`/projects/${result.id}/chat`);
            }
          }}
        >
          <Link2 size={14} />
          合并所选会话 ({selected.size})
        </button>
      </div>
      <div className="operation-legend">
        <span>
          <Play size={12} />
          Resume：恢复原长会话
        </span>
        <span>
          <Split size={12} />
          Fork：继承原历史分叉
        </span>
        <span className="primary">
          <Sparkles size={12} />
          Fresh Continuation：压缩后开启干净会话
        </span>
      </div>
      {loading && !sessions.length ? (
        <LoadingState label="正在读取真实会话索引" />
      ) : error ? (
        <ErrorState message={error} onRetry={() => void loadSessions()} />
      ) : !visible.length ? (
        <EmptyState
          icon={<RadioTower size={23} />}
          title="未发现 Codex 会话"
          detail="确认设置中的 sessions 目录后重新扫描；Continuum 不会生成演示会话。"
          action={
            <button
              className="button button-primary"
              onClick={() => void scanSessions()}
            >
              扫描默认目录
            </button>
          }
        />
      ) : (
        <div className="source-session-list">
          {renderedSessions.map((session) => (
            <article
              key={session.id}
              className={selected.has(session.id) ? "selected" : ""}
            >
              <label className="session-check">
                <input
                  type="checkbox"
                  checked={selected.has(session.id)}
                  onChange={() => toggle(session.id)}
                />
                <span />
              </label>
              <div className="source-session-main">
                <button
                  className="session-title"
                  onClick={() => navigate(`/sessions/${session.id}`)}
                >
                  <strong>{session.title}</strong>
                </button>
                <div className="source-session-meta">
                  <Badge tone="signal">{clientLabel(session)}</Badge>
                  <span>
                    <FolderKanban size={12} />
                    {session.boundProjectName ?? workspaceName(session.workingDirectory)}
                  </span>
                  <span>
                    {new Date(session.updatedAt).toLocaleString("zh-CN")}
                  </span>
                  <span className={session.boundProjectId ? "binding-status bound" : "binding-status"}>
                    <CheckCircle2 size={12} />
                    {session.boundProjectId ? "已绑定" : "未绑定"}
                  </span>
                </div>
              </div>
              <div className="session-operations">
                <button
                  className="button button-primary"
                  onClick={() => void fresh(session)}
                  disabled={!projectId && !session.boundProjectId}
                >
                  <Sparkles size={13} />
                  新建续接
                </button>
                <details className="session-overflow">
                  <summary className="icon-button" aria-label={`${session.title}的更多操作`}>
                    <MoreHorizontal size={16} />
                  </summary>
                  <div className="session-overflow-menu">
                    <button onClick={() => void launchSource(session, "resume")}>
                      <Play size={13} />继续原会话
                    </button>
                    <button onClick={() => void launchSource(session, "fork")}>
                      <Split size={13} />从此处分支
                    </button>
                    <button onClick={() => navigate(`/sessions/${session.id}`)}>
                      查看详情
                    </button>
                    <button onClick={async () => {
                      await navigator.clipboard.writeText(session.sourcePath);
                      notify({ tone: "success", title: "会话路径已复制" });
                    }}>
                      <Clipboard size={13} />复制会话路径
                    </button>
                  </div>
                </details>
              </div>
            </article>
          ))}
          {renderedSessions.length < visible.length && (
            <button
              className="load-earlier"
              onClick={() => setVisibleCount((count) => count + 30)}
            >
              加载更多（已显示 {renderedSessions.length} / {visible.length}）
            </button>
          )}
        </div>
      )}
    </div>
  );
}
