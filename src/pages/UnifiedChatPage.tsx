import {
  Activity,
  ArrowUpRight,
  BrainCircuit,
  GitFork,
  Braces,
  CheckSquare2,
  Clipboard,
  Copy,
  Code2,
  FileCode2,
  Files,
  GitBranch,
  Link2,
  MessageSquarePlus,
  MessagesSquare,
  MoreHorizontal,
  Pin,
  RadioTower,
  RefreshCw,
  Route,
  Search,
  ServerCog,
  Sparkles,
  Terminal,
  Waypoints,
  Workflow,
  X,
} from "lucide-react";
import { useGSAP } from "@gsap/react";
import gsap from "gsap";
import { Flip } from "gsap/Flip";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
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

gsap.registerPlugin(useGSAP, Flip);

type WorkspaceView = "chat" | "sessions" | "graph" | "context" | "activity" | "files";
type InspectorKind = "context" | "git" | "skills" | "diagnostics" | null;

const workspaceViews: Array<{
  id: WorkspaceView;
  label: string;
  icon: typeof MessagesSquare;
}> = [
  { id: "chat", label: "Chat", icon: MessagesSquare },
  { id: "sessions", label: "Sessions", icon: RadioTower },
  { id: "graph", label: "Graph", icon: Workflow },
  { id: "context", label: "Context", icon: Waypoints },
  { id: "activity", label: "Activity", icon: Activity },
  { id: "files", label: "Files", icon: Files },
];

export default function UnifiedChatPage() {
  const { id = "" } = useParams();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const workspaceRef = useRef<HTMLDivElement>(null);
  const viewAnimating = useRef(false);
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
  const [activeView, setActiveView] = useState<WorkspaceView>("chat");
  const [inspector, setInspector] = useState<InspectorKind>(null);
  const [indexDrawerOpen, setIndexDrawerOpen] = useState(false);
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
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (inspector) setInspector(null);
      else if (indexDrawerOpen) setIndexDrawerOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [indexDrawerOpen, inspector]);

  const { contextSafe } = useGSAP(
    () => undefined,
    { scope: workspaceRef },
  );

  const switchWorkspaceView = contextSafe((nextView: WorkspaceView) => {
    if (nextView === activeView || viewAnimating.current) return;
    const reduced = window.matchMedia?.(
      "(prefers-reduced-motion: reduce)",
    )?.matches;
    const targets = workspaceRef.current?.querySelectorAll("[data-flip-region]");
    const firstBounds = targets?.[0]?.getBoundingClientRect();
    if (
      reduced ||
      !targets?.length ||
      (firstBounds?.width === 0 && firstBounds.height === 0)
    ) {
      setActiveView(nextView);
      return;
    }
    viewAnimating.current = true;
    gsap.killTweensOf(targets);
    const state = Flip.getState(targets, { props: "opacity" });
    flushSync(() => setActiveView(nextView));
    Flip.from(state, {
      duration: 0.62,
      ease: "power3.inOut",
      absolute: true,
      nested: true,
      stagger: { amount: 0.05, from: "center" },
      onComplete: () => {
        viewAnimating.current = false;
        const content = workspaceRef.current?.querySelectorAll(
          ".workspace-view-content > *",
        );
        if (content?.length) {
          gsap.fromTo(
            content,
            { autoAlpha: 0, y: 8 },
            {
              autoAlpha: 1,
              y: 0,
              duration: 0.28,
              stagger: 0.025,
              overwrite: true,
              clearProps: "transform,opacity,visibility",
            },
          );
        }
      },
      onInterrupt: () => {
        viewAnimating.current = false;
      },
    });
  });

  useGSAP(
    () => {
      const panel = workspaceRef.current?.querySelector(".workspace-inspector");
      if (!panel || !inspector) return;
      const reduced = window.matchMedia?.(
        "(prefers-reduced-motion: reduce)",
      )?.matches;
      gsap.killTweensOf(panel);
      const isBottomDrawer = inspector !== "context";
      gsap.fromTo(
        panel,
        {
          xPercent: reduced || isBottomDrawer ? 0 : 104,
          yPercent: reduced || !isBottomDrawer ? 0 : 104,
          autoAlpha: reduced ? 1 : 0.7,
        },
        {
          xPercent: 0,
          yPercent: 0,
          autoAlpha: 1,
          duration: reduced ? 0 : 0.5,
          ease: "power4.out",
          overwrite: true,
          clearProps: "transform,opacity,visibility",
        },
      );
      gsap.fromTo(
        panel.querySelectorAll(".workspace-inspector-body > *"),
        { autoAlpha: reduced ? 1 : 0, y: reduced ? 0 : 12 },
        {
          autoAlpha: 1,
          y: 0,
          duration: reduced ? 0 : 0.36,
          ease: "power3.out",
          stagger: reduced ? 0 : 0.055,
          delay: reduced ? 0 : 0.12,
          overwrite: true,
          clearProps: "transform,opacity,visibility",
        },
      );
    },
    { dependencies: [inspector], scope: workspaceRef, revertOnUpdate: true },
  );
  useGSAP(
    () => {
      if (!indexDrawerOpen) return;
      const drawer = workspaceRef.current?.querySelector(".archive-index-drawer");
      if (!drawer) return;
      const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
      gsap.killTweensOf(drawer);
      gsap.fromTo(
        drawer,
        { yPercent: reduced ? 0 : 108, autoAlpha: reduced ? 1 : 0.75 },
        {
          yPercent: 0,
          autoAlpha: 1,
          duration: reduced ? 0 : 0.58,
          ease: "power4.out",
          overwrite: true,
        },
      );
      gsap.fromTo(
        drawer.querySelectorAll("section, .rail-heading > *"),
        { autoAlpha: reduced ? 1 : 0, y: reduced ? 0 : 14 },
        {
          autoAlpha: 1,
          y: 0,
          duration: reduced ? 0 : 0.38,
          ease: "power3.out",
          stagger: reduced ? 0 : 0.06,
          delay: reduced ? 0 : 0.14,
          overwrite: true,
          clearProps: "transform,opacity,visibility",
        },
      );
    },
    { dependencies: [indexDrawerOpen], scope: workspaceRef, revertOnUpdate: true },
  );

  const closeIndexDrawer = contextSafe(() => {
    const drawer = workspaceRef.current?.querySelector(".archive-index-drawer");
    const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
    if (!drawer || reduced) {
      setIndexDrawerOpen(false);
      return;
    }
    gsap.killTweensOf(drawer);
    gsap.to(drawer, {
      yPercent: 108,
      autoAlpha: 0.75,
      duration: 0.46,
      ease: "power3.inOut",
      overwrite: true,
      onComplete: () => setIndexDrawerOpen(false),
    });
  });
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
    setIndexDrawerOpen(false);
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
    <div ref={workspaceRef} className={`unified-chat workspace-view-${activeView}`}>
      <aside
        className={`conversation-rail archive-index-drawer ${indexDrawerOpen ? "is-open" : ""}`}
        data-flip-region
        aria-hidden={!indexDrawerOpen}
      >
        <div className="rail-heading">
          <div>
            <button onClick={() => navigate("/projects")}>PROJECT INDEX</button>
            <strong>{project.name}</strong>
            <PathText value={project.projectPath} />
          </div>
          <button className="archive-drawer-close" aria-label="关闭项目索引" onClick={closeIndexDrawer}><X size={17} /></button>
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
      {indexDrawerOpen && (
        <button
          className="archive-drawer-scrim"
          aria-label="关闭项目索引"
          onClick={closeIndexDrawer}
        />
      )}
      <main className="conversation-main" data-flip-region>
        <header className="conversation-top">
          <div>
            <div className="conversation-project-index">
              <button onClick={() => setIndexDrawerOpen(true)}>
                Project index <span>{project.branches.length + project.sessions.length}</span>
              </button>
              <label className="archive-branch-select">
                <GitBranch size={13} />
                <span>Branch</span>
                <select value={branchId} onChange={(event) => void switchBranch(event.target.value)}>
                  {project.branches.filter((branch) => branch.status === "active").map((branch) => (
                    <option value={branch.id} key={branch.id}>{branch.name}</option>
                  ))}
                </select>
              </label>
            </div>
            <div className="conversation-title">
              <h1>{project.name}</h1>
              <HealthBadge level={project.health.level} />
            </div>
            <p>{project.currentTask || project.goal}</p>
          </div>
          <div className="conversation-actions">
            <button
              className="button button-secondary"
              onClick={() => switchWorkspaceView("context")}
            >
              <Waypoints size={14} />
              Context
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
        <nav className="workspace-view-nav" aria-label="项目视图">
          <div className="workspace-view-tabs">
            {workspaceViews.map(({ id: viewId, label, icon: Icon }) => (
              <button
                key={viewId}
                className={activeView === viewId ? "active" : ""}
                aria-current={activeView === viewId ? "page" : undefined}
                onClick={() => switchWorkspaceView(viewId)}
              >
                <Icon size={14} />
                {label}
              </button>
            ))}
          </div>
          <div className="workspace-inspector-actions">
            <button onClick={() => setInspector("git")}><GitBranch size={13} />Git</button>
            <button onClick={() => setInspector("skills")}><ServerCog size={13} />Skills / MCP</button>
            <button onClick={() => setInspector("diagnostics")}><BrainCircuit size={13} />Diagnostics</button>
          </div>
        </nav>
        {activeView === "chat" ? (
          <>
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
          </>
        ) : (
          <ProjectWorkspaceView
            view={activeView}
            project={project}
            nodes={nodes}
            onOpenSession={(sessionId) => navigate(`/sessions/${sessionId}`)}
            onOpenContext={() => navigate(`/projects/${id}/context?branch=${branchId}`)}
          />
        )}
      </main>
      {inspector && (
        <>
          <button
            className="workspace-inspector-scrim"
            aria-label="关闭检查面板"
            onClick={() => setInspector(null)}
          />
          <WorkspaceInspector
            kind={inspector}
            project={project}
            onClose={() => setInspector(null)}
            onNavigate={(path) => navigate(path)}
          />
        </>
      )}
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

function ProjectWorkspaceView({
  view,
  project,
  nodes,
  onOpenSession,
  onOpenContext,
}: {
  view: Exclude<WorkspaceView, "chat">;
  project: UnifiedProjectDetail;
  nodes: ConversationNode[];
  onOpenSession: (sessionId: string) => void;
  onOpenContext: () => void;
}) {
  if (view === "sessions") {
    return (
      <section className="workspace-view-content session-chain-view" data-flip-region>
        <header><div><p className="eyebrow">SOURCE CHAIN</p><h2>来源会话</h2></div><span>{project.sessions.length} THREADS</span></header>
        <div className="session-chain">
          {project.sessions.map((session, index) => (
            <button key={session.id} onClick={() => onOpenSession(session.id)}>
              <span className="chain-index">{String(index + 1).padStart(2, "0")}</span>
              <i aria-hidden="true" />
              <span><strong>{session.title}</strong><small>{session.messageCount} messages · {new Date(session.lastSyncedAt).toLocaleString("zh-CN")}</small></span>
              <code>{session.continuationId ? "FRESH" : "SOURCE"}</code>
              <ArrowUpRight size={15} />
            </button>
          ))}
          {!project.sessions.length && <p className="workspace-empty">当前项目尚未绑定来源会话。</p>}
        </div>
      </section>
    );
  }

  if (view === "graph") {
    return (
      <section className="workspace-view-content graph-workspace-view" data-flip-region>
        <header><div><p className="eyebrow">CONVERSATION GRAPH</p><h2>分支与会话链</h2></div><span>{nodes.length} NODES</span></header>
        <div className="branch-graph" role="img" aria-label="对话分支和会话节点关系">
          {project.branches.map((branch, branchIndex) => {
            const branchNodes = nodes.filter((node) => node.branchId === branch.id);
            const branchSessions = project.sessions.filter((session) => session.branchId === branch.id);
            return (
              <article key={branch.id} className={branch.id === project.currentBranchId ? "active" : ""}>
                <div className="branch-axis"><span>{String(branchIndex + 1).padStart(2, "0")}</span><i /></div>
                <div className="branch-graph-body">
                  <header><GitBranch size={14} /><strong>{branch.name}</strong><code>{branch.status}</code></header>
                  <div className="branch-session-nodes">
                    {branchSessions.map((session) => <button key={session.id} onClick={() => onOpenSession(session.id)}><RadioTower size={13} /><span>{session.title}</span></button>)}
                    {!branchSessions.length && <span className="graph-placeholder">NO SOURCE SESSION</span>}
                  </div>
                  <footer><span>{branchNodes.length} indexed nodes</span><span>{branch.forkNodeId ? `fork ${branch.forkNodeId.slice(0, 8)}` : "root branch"}</span></footer>
                </div>
              </article>
            );
          })}
        </div>
        <p className="graph-note">图使用 SQLite 中的真实 branch、session 与 node 关系；选择节点合并仍在后续完整流程中。</p>
      </section>
    );
  }

  if (view === "context") {
    return (
      <section className="workspace-view-content context-workspace-view" data-flip-region>
        <header><div><p className="eyebrow">CONTEXT COMPILER</p><h2>当前可携带上下文</h2></div><span>{Math.round(project.health.thresholdRatio * 100)}% BUDGET</span></header>
        <div className="context-desk-grid">
          <section className="context-desk-lead">
            <span>Current task</span>
            <h3>{project.currentTask || project.goal}</h3>
            <p>{project.goal}</p>
            <div className="context-budget-line"><i style={{ width: `${Math.min(100, Math.round(project.health.thresholdRatio * 100))}%` }} /></div>
            <div><HealthBadge level={project.health.level} /><strong>{project.health.estimatedTokens.toLocaleString()} estimated tokens</strong></div>
          </section>
          <section>
            <span>Constraints</span>
            <ul>{project.constraints.length ? project.constraints.map((constraint) => <li key={constraint}>{constraint}</li>) : <li>没有项目级长期约束</li>}</ul>
          </section>
          <section>
            <span>Decisions</span>
            <ul>{project.decisions.length ? project.decisions.slice(-6).map((decision) => <li key={decision.id}>{decision.content}</li>) : <li>尚未提取稳定决策</li>}</ul>
          </section>
          <section>
            <span>Health signals</span>
            <ul>{project.health.reasons.length ? project.health.reasons.map((reason) => <li key={reason}>{reason}</li>) : <li>上下文状态稳定</li>}</ul>
          </section>
        </div>
        <footer className="context-desk-actions">
          <p>完整 Inspector 可查看快照、逐项决策和 Diff。</p>
          <button className="button button-primary" onClick={onOpenContext}>打开完整 Context Inspector</button>
        </footer>
      </section>
    );
  }

  if (view === "activity") {
    const activityNodes = nodes.filter((node) => node.nodeType !== "message").slice(-120).reverse();
    return (
      <section className="workspace-view-content activity-workspace-view" data-flip-region>
        <header><div><p className="eyebrow">PROJECT ACTIVITY</p><h2>执行活动</h2></div><span>{activityNodes.length} EVENTS</span></header>
        <div className="activity-ledger">
          {activityNodes.map((node) => (
            <article key={node.id}>
              <time>{new Date(node.createdAt).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}</time>
              <span className={`activity-kind kind-${node.nodeType}`}>{node.nodeType}</span>
              <p>{node.content}</p>
              <code>{node.sourceSessionId?.slice(0, 8) || "continuum"}</code>
            </article>
          ))}
          {!activityNodes.length && <p className="workspace-empty">当前分支没有工具、文件或切换活动。</p>}
        </div>
      </section>
    );
  }

  const git = project.gitState;
  const files = Array.from(new Set([
    ...project.activeFiles,
    ...(git?.modified ?? []),
    ...(git?.staged ?? []),
    ...(git?.untracked ?? []),
  ]));
  return (
    <section className="workspace-view-content files-workspace-view" data-flip-region>
      <header><div><p className="eyebrow">READ-ONLY WORKSPACE</p><h2>Files</h2></div><span>{files.length} PATHS</span></header>
      <div className="file-workspace-grid">
        <div className="file-index">
          {files.map((file) => <div key={file}><FileCode2 size={14} /><PathText value={file} /><span>{git?.staged.includes(file) ? "STAGED" : git?.untracked.includes(file) ? "NEW" : git?.modified.includes(file) ? "MODIFIED" : "ACTIVE"}</span></div>)}
          {!files.length && <p className="workspace-empty">尚未从时间线或 Git 读取到活跃文件。</p>}
        </div>
        <aside>
          <p className="eyebrow">GIT SNAPSHOT</p>
          <strong>{git?.branch || "Not a repository"}</strong>
          <code>{git?.head || "—"}</code>
          <dl><div><dt>Modified</dt><dd>{git?.modified.length ?? 0}</dd></div><div><dt>Staged</dt><dd>{git?.staged.length ?? 0}</dd></div><div><dt>Untracked</dt><dd>{git?.untracked.length ?? 0}</dd></div></dl>
          <button className="button button-secondary" onClick={onOpenContext}>查看上下文中的 Git 摘要</button>
        </aside>
      </div>
    </section>
  );
}

function WorkspaceInspector({
  kind,
  project,
  onClose,
  onNavigate,
}: {
  kind: Exclude<InspectorKind, null>;
  project: UnifiedProjectDetail;
  onClose: () => void;
  onNavigate: (path: string) => void;
}) {
  const labels = {
    context: ["CONTEXT INSPECTOR", "上下文与健康"],
    git: ["READ-ONLY GIT", "工作树快照"],
    skills: ["PROJECT CAPABILITIES", "Skills / MCP"],
    diagnostics: ["LOCAL DIAGNOSTICS", "运行状态"],
  } as const;
  return (
    <aside className={`workspace-inspector inspector-${kind}`} role="dialog" aria-modal="true" aria-label={labels[kind][1]}>
      <header><div><p>{labels[kind][0]}</p><h2>{labels[kind][1]}</h2></div><button aria-label="关闭检查面板" onClick={onClose}><X size={17} /></button></header>
      <div className="workspace-inspector-body">
        {kind === "context" && <>
          <section className="inspector-lead"><span>预算占用</span><strong>{Math.round(project.health.thresholdRatio * 100)}%</strong><HealthBadge level={project.health.level} /></section>
          <section><p className="eyebrow">CURRENT GOAL</p><h3>{project.currentTask || "总体目标"}</h3><p>{project.goal}</p></section>
          <section><p className="eyebrow">HEALTH SIGNALS</p><dl><div><dt>估算 Token</dt><dd>{project.health.estimatedTokens.toLocaleString()}</dd></div><div><dt>重复比例</dt><dd>{Math.round(project.health.duplicateRatio * 100)}%</dd></div><div><dt>冲突</dt><dd>{project.health.conflictCount}</dd></div></dl>{project.health.reasons.map((reason) => <p className="inspector-reason" key={reason}>{reason}</p>)}</section>
          <section><p className="eyebrow">CONSTRAINTS</p><ul>{project.constraints.map((constraint) => <li key={constraint}>{constraint}</li>)}</ul></section>
          <button className="button button-primary button-wide" onClick={() => onNavigate(`/projects/${project.id}/context?branch=${project.currentBranchId}`)}>打开完整 Context Inspector</button>
        </>}
        {kind === "git" && <>
          <section className="inspector-lead"><span>当前分支</span><strong className="inspector-branch">{project.gitState?.branch || "—"}</strong><code>{project.gitState?.head || "NO HEAD"}</code></section>
          <section><p className="eyebrow">WORKTREE</p><dl><div><dt>Modified</dt><dd>{project.gitState?.modified.length ?? 0}</dd></div><div><dt>Staged</dt><dd>{project.gitState?.staged.length ?? 0}</dd></div><div><dt>Untracked</dt><dd>{project.gitState?.untracked.length ?? 0}</dd></div></dl></section>
          <section><p className="eyebrow">MODIFIED PATHS</p>{project.gitState?.modified.map((file) => <PathText key={file} value={file} />) || <p>未读取 Git 状态。</p>}</section>
          <p className="inspector-footnote">此面板只展示 Context Compiler 已读取的 Git 快照，不执行 Git 命令或修改。</p>
        </>}
        {kind === "skills" && <>
          <section className="inspector-lead"><span>绑定范围</span><strong className="inspector-branch">PROJECT</strong><code>{project.name}</code></section>
          <section><p className="eyebrow">CAPABILITY BOUNDARY</p><h3>配置参与下一次编译</h3><p>项目绑定的 Skills 与 MCP 会进入 Context Compiler 摘要；不会自动改写第三方配置。</p></section>
          <button className="button button-primary button-wide" onClick={() => onNavigate(`/configurations?project=${project.id}`)}>查看 Skills 与 MCP</button>
        </>}
        {kind === "diagnostics" && <>
          <section className="inspector-lead"><span>数据路径</span><strong className="inspector-branch">LOCAL</strong><code>SQLite v4</code></section>
          <section><p className="eyebrow">RUNTIME PATHS</p><p>Fresh 优先通过 Codex App Server；配置无法无损映射时使用 CLI fallback。</p></section>
          <section><p className="eyebrow">CURRENT PROJECT</p><dl><div><dt>Sessions</dt><dd>{project.sessionCount}</dd></div><div><dt>Branches</dt><dd>{project.branches.length}</dd></div><div><dt>Path</dt><dd>{project.pathExists ? "READY" : "MISSING"}</dd></div></dl></section>
          <button className="button button-primary button-wide" onClick={() => onNavigate("/diagnostics")}>运行完整诊断</button>
        </>}
      </div>
    </aside>
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
