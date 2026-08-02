import {
  Clipboard,
  GitBranch,
  Link2,
  Play,
  RadioTower,
  RefreshCw,
  Search,
  Sparkles,
  Split,
  Waypoints,
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
  PathText,
} from "../components/ui";
import { getAgentLabel } from "../config/agents";
import { useAppStore } from "../store/appStore";
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
    scanSessions,
    notify,
  } = useAppStore();
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
          `${item.title} ${item.id} ${item.workingDirectory ?? ""}`
            .toLowerCase()
            .includes(query.toLowerCase()),
        )
        .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)),
    [sessions, query],
  );
  useEffect(() => setVisibleCount(30), [query]);
  const renderedSessions = visible.slice(0, visibleCount);
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
            placeholder="搜索标题、Session ID 或工作目录"
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
                  <code>{session.id}</code>
                </button>
                <div className="source-session-meta">
                  <Badge tone="signal">{getAgentLabel(session.agent)}</Badge>
                  <span>
                    <GitBranch size={12} />
                    <PathText value={session.workingDirectory} />
                  </span>
                  <span>
                    {session.messageCount} 消息 · {session.toolCallCount} 工具
                  </span>
                  <span>
                    {new Date(session.updatedAt).toLocaleString("zh-CN")}
                  </span>
                </div>
              </div>
              <div className="session-operations">
                <button
                  className="button button-quiet"
                  onClick={() => void launchSource(session, "resume")}
                >
                  <Play size={13} />
                  继续原会话
                </button>
                <button
                  className="button button-primary"
                  onClick={() => void fresh(session)}
                  disabled={!projectId}
                >
                  <Sparkles size={13} />
                  压缩后新建会话
                </button>
                <button
                  className="button button-quiet"
                  onClick={() => void launchSource(session, "fork")}
                >
                  <Split size={13} />
                  从此处分支
                </button>
                <span className="future-agent-label">
                  <Waypoints size={13} />
                  其他 Agent：未来扩展
                </span>
                <button
                  className="icon-button"
                  title="复制会话路径"
                  onClick={async () => {
                    await navigator.clipboard.writeText(session.sourcePath);
                    notify({ tone: "success", title: "会话路径已复制" });
                  }}
                >
                  <Clipboard size={14} />
                </button>
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
