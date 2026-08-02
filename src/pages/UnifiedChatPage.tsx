import {
  GitFork,
  Braces,
  CheckSquare2,
  Clipboard,
  Copy,
  Code2,
  FileCode2,
  GitBranch,
  Link2,
  MessageSquarePlus,
  MoreHorizontal,
  Pin,
  RadioTower,
  RefreshCw,
  Route,
  Search,
  Sparkles,
  Terminal,
  Waypoints,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { appApi } from "../api/bridge";
import {
  Badge,
  ConfirmDialog,
  ErrorState,
  LoadingState,
  PathText,
} from "../components/ui";
import { getAgentLabel } from "../config/agents";
import { useAppStore } from "../store/appStore";
import type {
  ConversationBranch,
  ConversationNode,
  UnifiedProjectDetail,
} from "../types/models";
import { HealthBadge } from "./ProjectsPage";

export default function UnifiedChatPage() {
  const { id = "" } = useParams();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const notify = useAppStore((state) => state.notify);
  const timelineRevision = useAppStore((state) => state.timelineRevision);
  const [project, setProject] = useState<UnifiedProjectDetail | null>(null);
  const [nodes, setNodes] = useState<ConversationNode[]>([]);
  const [branchId, setBranchId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState("");
  const [syncing, setSyncing] = useState(false);
  const [branchFrom, setBranchFrom] = useState<ConversationNode | null>(null);
  const [branchName, setBranchName] = useState("");
  const [timelineQuery, setTimelineQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState("all");
  const [sessionFilter, setSessionFilter] = useState("all");
  const [visibleCount, setVisibleCount] = useState(100);
  const [manageBranch, setManageBranch] = useState<ConversationBranch | null>(
    null,
  );
  const [manageBranchName, setManageBranchName] = useState("");
  const [deleteBranch, setDeleteBranch] = useState<ConversationBranch | null>(
    null,
  );
  const requestedBranch = searchParams.get("branch");
  const requestedNode = searchParams.get("node");
  const load = useCallback(async () => {
    try {
      const value = await appApi.project(id);
      const activeBranch = branchId || requestedBranch || value.currentBranchId;
      setProject(value);
      setBranchId(activeBranch);
      setNodes(await appApi.timeline(id, activeBranch));
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, [id, branchId, requestedBranch]);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    if (timelineRevision > 0 && branchId)
      void appApi.timeline(id, branchId).then(setNodes);
  }, [id, branchId, timelineRevision]);
  const groupedSessions = useMemo(
    () =>
      new Map(project?.sessions.map((session) => [session.id, session]) ?? []),
    [project],
  );
  const filteredNodes = useMemo(
    () =>
      nodes.filter(
        (node) =>
          (typeFilter === "all" || node.nodeType === typeFilter) &&
          (sessionFilter === "all" || node.sourceSessionId === sessionFilter) &&
          `${node.content} ${node.nodeType} ${node.status}`
            .toLowerCase()
            .includes(timelineQuery.toLowerCase()),
      ),
    [nodes, sessionFilter, timelineQuery, typeFilter],
  );
  const displayedNodes = filteredNodes.slice(-visibleCount);
  const nodeTypes = useMemo(
    () => Array.from(new Set(nodes.map((node) => node.nodeType))).sort(),
    [nodes],
  );
  useEffect(() => {
    if (!requestedNode || !nodes.length) return;
    setVisibleCount(nodes.length);
    requestAnimationFrame(() => {
      const target = document.getElementById(`node-${requestedNode}`);
      if (typeof target?.scrollIntoView === "function") {
        target.scrollIntoView({ block: "center" });
      }
    });
  }, [nodes, requestedNode]);
  async function sync() {
    setSyncing(true);
    try {
      const result = await appApi.pollSessionChanges();
      await load();
      notify({
        tone: "success",
        title: "会话已增量同步",
        detail: `${result.insertedNodes} 个新增节点`,
      });
    } finally {
      setSyncing(false);
    }
  }
  async function addNote() {
    if (!note.trim()) return;
    await appApi.addNote(id, branchId, note);
    setNote("");
    await load();
  }
  async function createBranch() {
    if (!branchFrom || !branchName.trim()) return;
    const branch = await appApi.createBranch(id, branchFrom.id, branchName);
    setBranchFrom(null);
    setBranchName("");
    setBranchId(branch.id);
    notify({ tone: "success", title: "对话分支已创建", detail: branch.name });
  }
  async function switchBranch(nextBranchId: string) {
    await appApi.switchBranch(id, nextBranchId);
    setBranchId(nextBranchId);
    setVisibleCount(100);
  }
  async function saveBranchName() {
    if (!manageBranch || !manageBranchName.trim()) return;
    await appApi.renameBranch(manageBranch.id, manageBranchName.trim());
    setManageBranch(null);
    await load();
  }
  async function toggleBranchArchive() {
    if (!manageBranch) return;
    if (manageBranch.status === "archived")
      await appApi.restoreBranch(manageBranch.id);
    else await appApi.archiveBranch(manageBranch.id);
    setManageBranch(null);
    setBranchId("");
    await load();
  }
  async function confirmDeleteBranch() {
    if (!deleteBranch) return;
    try {
      await appApi.deleteBranch(deleteBranch.id);
      setDeleteBranch(null);
      setManageBranch(null);
      setBranchId("");
      await load();
      notify({ tone: "success", title: "空分支记录已删除" });
    } catch (reason) {
      setDeleteBranch(null);
      notify({
        tone: "error",
        title: "分支未删除",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    }
  }
  async function copyContext() {
    const compiled = await appApi.compileContext(
      defaultOptions(project!, branchId, nodes.at(-1)?.id ?? null),
    );
    await navigator.clipboard.writeText(compiled.compiledText);
    notify({
      tone: "success",
      title: "续接上下文已复制",
      detail: `约 ${compiled.estimatedTokens} tokens`,
    });
  }
  async function copyNode(node: ConversationNode) {
    await navigator.clipboard.writeText(node.content);
    notify({ tone: "success", title: "节点内容已复制", detail: node.id });
  }
  async function pinNode(node: ConversationNode) {
    await appApi.updateNode(node.id, "active", 100);
    await load();
    notify({ tone: "success", title: "节点已固定到高优先级上下文" });
  }
  if (error)
    return (
      <div className="page">
        <ErrorState message={error} onRetry={() => void load()} />
      </div>
    );
  if (!project) return <LoadingState label="正在组装统一项目对话" />;
  return (
    <div className="unified-chat">
      <aside className="conversation-rail">
        <div className="rail-heading">
          <button onClick={() => navigate("/projects")}>CONTINUUM</button>
          <strong>{project.name}</strong>
          <PathText value={project.projectPath} />
        </div>
        <section>
          <p className="eyebrow">BRANCHES</p>
          {project.branches.map((branch) => (
            <div className="rail-branch" key={branch.id}>
              <button
                className={`rail-row ${branch.id === branchId ? "active" : ""}`}
                onClick={() => void switchBranch(branch.id)}
                disabled={branch.status !== "active"}
              >
                <GitBranch size={14} />
                <span>{branch.name}</span>
                <small>
                  {branch.status === "active" ? branch.nodeCount : "已归档"}
                </small>
              </button>
              <button
                className="branch-more"
                title={`管理分支 ${branch.name}`}
                onClick={() => {
                  setManageBranch(branch);
                  setManageBranchName(branch.name);
                }}
              >
                <MoreHorizontal size={13} />
              </button>
            </div>
          ))}
        </section>
        <section>
          <p className="eyebrow">SOURCE SESSIONS</p>
          {project.sessions.map((session) => (
            <div className="rail-row session" key={session.id}>
              <RadioTower size={14} />
              <span title={session.id}>{session.title}</span>
              <small>{session.messageCount}</small>
            </div>
          ))}
          {!project.sessions.length && (
            <p className="rail-empty">尚未绑定会话</p>
          )}
          <button
            className="rail-add"
            onClick={() => navigate(`/sessions?project=${id}`)}
          >
            <Link2 size={13} />
            绑定已有会话
          </button>
        </section>
      </aside>
      <main className="conversation-main">
        <header className="conversation-top">
          <div>
            <div className="conversation-title">
              <h1>{project.name}</h1>
              <HealthBadge level={project.health.level} />
            </div>
            <p>{project.currentTask || project.goal}</p>
          </div>
          <div className="conversation-actions">
            <button
              className="button button-secondary"
              onClick={() =>
                navigate(`/projects/${id}/context?branch=${branchId}`)
              }
            >
              <Waypoints size={14} />
              上下文检查
            </button>
            <button
              className="button button-secondary"
              onClick={() => void sync()}
              disabled={syncing}
            >
              <RefreshCw size={14} className={syncing ? "animate-spin" : ""} />
              同步
            </button>
            <button
              className="button button-primary fresh-button"
              onClick={() =>
                navigate(
                  `/projects/${id}/continuation?branch=${branchId}&node=${nodes.at(-1)?.id ?? ""}`,
                )
              }
            >
              <Sparkles size={15} />
              Fresh Continuation
            </button>
          </div>
        </header>
        <div className="timeline-toolbar">
          <label className="search-field">
            <Search size={14} />
            <input
              value={timelineQuery}
              onChange={(event) => setTimelineQuery(event.target.value)}
              placeholder="在当前分支中搜索"
            />
          </label>
          <select
            value={typeFilter}
            onChange={(event) => setTypeFilter(event.target.value)}
            aria-label="节点类型"
          >
            <option value="all">全部类型</option>
            {nodeTypes.map((type) => (
              <option value={type} key={type}>
                {type}
              </option>
            ))}
          </select>
          <select
            value={sessionFilter}
            onChange={(event) => setSessionFilter(event.target.value)}
            aria-label="来源会话"
          >
            <option value="all">全部会话</option>
            {project.sessions.map((session) => (
              <option value={session.id} key={session.id}>
                {session.title}
              </option>
            ))}
          </select>
          <span>
            {filteredNodes.length} / {nodes.length}
          </span>
        </div>
        <div className="timeline-stream">
          {filteredNodes.length ? (
            <>
              {displayedNodes.length < filteredNodes.length && (
                <button
                  className="load-earlier"
                  onClick={() => setVisibleCount((value) => value + 100)}
                >
                  加载更早的 100 条节点
                </button>
              )}
              {displayedNodes.map((node) => (
                <TimelineNode
                  key={node.id}
                  node={node}
                  sessionTitle={
                    node.sourceSessionId
                      ? groupedSessions.get(node.sourceSessionId)?.title
                      : undefined
                  }
                  onBranch={() => {
                    setBranchFrom(node);
                    setBranchName(`branch-${project.branches.length + 1}`);
                  }}
                  onContinue={() =>
                    navigate(
                      `/projects/${id}/continuation?branch=${branchId}&node=${node.id}`,
                    )
                  }
                  onCopy={() => void copyNode(node)}
                  onPin={() => void pinNode(node)}
                />
              ))}
            </>
          ) : (
            <div className="timeline-empty">
              <Route size={26} />
              <h2>当前分支还没有会话节点</h2>
              <p>绑定真实 Codex 会话后，来源消息和切换点会显示在这里。</p>
              <button
                className="button button-primary"
                onClick={() => navigate(`/sessions?project=${id}`)}
              >
                绑定会话
              </button>
            </div>
          )}
        </div>
        <footer className="composer">
          <textarea
            value={note}
            onChange={(event) => setNote(event.target.value)}
            placeholder="添加用户备注（不会发送给 Agent）"
            rows={2}
          />
          <div>
            <span>本地备注将成为可检索的 ConversationNode</span>
            <button className="text-button" onClick={() => void copyContext()}>
              <Clipboard size={13} />
              复制当前编译上下文
            </button>
            <button
              className="button button-secondary"
              onClick={() => setBranchFrom(nodes.at(-1) ?? null)}
              disabled={!nodes.length}
            >
              <GitFork size={14} />
              从这里分支
            </button>
            <button
              className="button button-primary"
              onClick={() => void addNote()}
              disabled={!note.trim()}
            >
              <MessageSquarePlus size={14} />
              添加备注
            </button>
          </div>
        </footer>
      </main>
      <aside className="context-rail">
        <section>
          <p className="eyebrow">CURRENT GOAL</p>
          <h2>总体目标</h2>
          <p>{project.goal}</p>
        </section>
        <section>
          <p className="eyebrow">CONTEXT HEALTH</p>
          <div className="health-number">
            <strong>{Math.round(project.health.thresholdRatio * 100)}%</strong>
            <span>预算风险</span>
          </div>
          <dl>
            <div>
              <dt>估算 Token</dt>
              <dd>{project.health.estimatedTokens.toLocaleString()}</dd>
            </div>
            <div>
              <dt>重复比例</dt>
              <dd>{Math.round(project.health.duplicateRatio * 100)}%</dd>
            </div>
            <div>
              <dt>工具日志</dt>
              <dd>{Math.round(project.health.toolLogRatio * 100)}%</dd>
            </div>
          </dl>
          <small>仅基于长度、重复与陈旧信息估算，不代表模型智力测量。</small>
        </section>
        <section>
          <p className="eyebrow">CONSTRAINTS</p>
          <ul>
            {project.constraints.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </section>
        <section>
          <p className="eyebrow">ACTIVE FILES</p>
          {project.activeFiles.length ? (
            project.activeFiles.map((file) => (
              <PathText key={file} value={file} />
            ))
          ) : (
            <p className="muted-copy">未提取到活跃文件</p>
          )}
        </section>
        <section>
          <p className="eyebrow">GIT STATE</p>
          <dl>
            <div>
              <dt>分支</dt>
              <dd>{project.gitState?.branch ?? "—"}</dd>
            </div>
            <div>
              <dt>修改</dt>
              <dd>{project.gitState?.modified.length ?? 0}</dd>
            </div>
            <div>
              <dt>未跟踪</dt>
              <dd>{project.gitState?.untracked.length ?? 0}</dd>
            </div>
          </dl>
        </section>
      </aside>
      {branchFrom && (
        <div
          className="dialog-backdrop"
          role="presentation"
          onMouseDown={() => setBranchFrom(null)}
        >
          <div
            className="dialog"
            role="dialog"
            aria-modal="true"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="dialog-mark continuum">
              <GitFork size={19} />
            </div>
            <h2>从这里创建分支</h2>
            <p>
              新分支以节点 {branchFrom.id.slice(0, 12)}{" "}
              为父节点，原分支保持不变。
            </p>
            <label>
              <span>分支名称</span>
              <input
                autoFocus
                value={branchName}
                onChange={(event) => setBranchName(event.target.value)}
              />
            </label>
            <div className="dialog-actions">
              <button
                className="button button-secondary"
                onClick={() => setBranchFrom(null)}
              >
                取消
              </button>
              <button
                className="button button-primary"
                onClick={() => void createBranch()}
              >
                创建分支
              </button>
            </div>
          </div>
        </div>
      )}
      {manageBranch && (
        <div
          className="dialog-backdrop"
          role="presentation"
          onMouseDown={() => setManageBranch(null)}
        >
          <div
            className="dialog"
            role="dialog"
            aria-modal="true"
            aria-label="管理对话分支"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="dialog-mark continuum">
              <GitBranch size={19} />
            </div>
            <h2>管理分支</h2>
            <p>
              归档和删除只影响 Continuum
              分支记录；删除会拒绝仍有关联来源链的分支。
            </p>
            <label>
              <span>分支名称</span>
              <input
                value={manageBranchName}
                onChange={(event) => setManageBranchName(event.target.value)}
              />
            </label>
            <div className="dialog-actions branch-dialog-actions">
              <button
                className="button button-secondary"
                onClick={() => setManageBranch(null)}
              >
                取消
              </button>
              <button
                className="button button-secondary"
                onClick={() => void toggleBranchArchive()}
              >
                {manageBranch.status === "archived" ? "恢复分支" : "归档分支"}
              </button>
              <button
                className="button button-danger"
                onClick={() => setDeleteBranch(manageBranch)}
              >
                删除空分支
              </button>
              <button
                className="button button-primary"
                onClick={() => void saveBranchName()}
                disabled={!manageBranchName.trim()}
              >
                保存名称
              </button>
            </div>
          </div>
        </div>
      )}
      <ConfirmDialog
        open={Boolean(deleteBranch)}
        title="删除这个空分支记录？"
        description="只有没有子分支、会话、配置或 Continuation 绑定的分支才能删除。Codex 源会话文件不会被删除。"
        confirmLabel="删除分支记录"
        destructive
        onConfirm={() => void confirmDeleteBranch()}
        onCancel={() => setDeleteBranch(null)}
      />
    </div>
  );
}

function TimelineNode({
  node,
  sessionTitle,
  onBranch,
  onContinue,
  onCopy,
  onPin,
}: {
  node: ConversationNode;
  sessionTitle?: string;
  onBranch: () => void;
  onContinue: () => void;
  onCopy: () => void;
  onPin: () => void;
}) {
  if (["session_switch", "summary", "branch_point"].includes(node.nodeType))
    return (
      <div
        id={`node-${node.id}`}
        className={`graph-event event-${node.nodeType}`}
      >
        <span />
        <div>
          {node.nodeType === "session_switch" ? (
            <RadioTower size={14} />
          ) : node.nodeType === "branch_point" ? (
            <GitFork size={14} />
          ) : (
            <Braces size={14} />
          )}
          <strong>{node.content}</strong>
          <small>{new Date(node.createdAt).toLocaleString("zh-CN")}</small>
        </div>
      </div>
    );
  const role = String(node.metadata.role ?? "event");
  const icon =
    node.nodeType === "tool_call" ? (
      <Terminal size={15} />
    ) : node.nodeType === "file_change" ? (
      <FileCode2 size={15} />
    ) : node.nodeType === "todo" ? (
      <CheckSquare2 size={15} />
    ) : (
      <Code2 size={15} />
    );
  return (
    <article
      id={`node-${node.id}`}
      className={`unified-node node-${node.nodeType} node-status-${node.status}`}
    >
      <div className="node-source">
        {icon}
        <span>
          {node.sourceAgent ? getAgentLabel(node.sourceAgent) : "Continuum"}
        </span>
        {node.sourceSessionId && (
          <code title={node.sourceSessionId}>
            {sessionTitle || node.sourceSessionId.slice(0, 10)}
          </code>
        )}
      </div>
      <div className="node-content">
        <header>
          <Badge
            tone={
              role === "user"
                ? "signal"
                : node.nodeType === "error"
                  ? "danger"
                  : "neutral"
            }
          >
            {role}
          </Badge>
          <time>{new Date(node.createdAt).toLocaleString("zh-CN")}</time>
          <span className="importance">I{node.importance}</span>
        </header>
        {node.nodeType === "tool_call" ? (
          <details>
            <summary>{node.content.split("\n")[0]}</summary>
            <pre>{node.content}</pre>
          </details>
        ) : (
          <p>{node.content}</p>
        )}
        <details className="node-raw">
          <summary>原始记录</summary>
          <pre>{JSON.stringify(node, null, 2)}</pre>
        </details>
        <div className="node-actions">
          <button onClick={onCopy}>
            <Copy size={12} />
            复制
          </button>
          <button onClick={onPin} disabled={node.importance >= 100}>
            <Pin size={12} />
            固定
          </button>
          <button onClick={onBranch}>
            <GitFork size={12} />
            从此处分支
          </button>
          <button onClick={onContinue}>
            <Sparkles size={12} />
            Fresh Continuation
          </button>
        </div>
      </div>
    </article>
  );
}

export function defaultOptions(
  project: UnifiedProjectDetail,
  branchId: string,
  sourceNodeId: string | null,
) {
  return {
    projectId: project.id,
    branchId,
    sourceNodeId,
    targetAgent: project.defaultAgent,
    targetModel: project.defaultModel,
    tokenBudget: project.health.contextBudget,
    recentRounds: 8,
    includeToolLogs: true,
    includeGitDiff: true,
    includeFailedAttempts: true,
    includeSkills: true,
    includeMcp: true,
  } as const;
}
