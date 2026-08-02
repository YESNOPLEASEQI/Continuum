import {
  ArrowLeft,
  Ban,
  Check,
  Database,
  Eye,
  Filter,
  Pin,
  RefreshCw,
  Save,
  Search,
  ShieldAlert,
  Unlink,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { appApi } from "../api/bridge";
import { Badge, ErrorState, LoadingState, PageHeader } from "../components/ui";
import { useAppStore } from "../store/appStore";
import type {
  CompiledContext,
  ContextCompileOptions,
  ContextItem,
  ContextItemAction,
  ContextSnapshot,
  ContextSnapshotDiff,
  UnifiedProjectDetail,
} from "../types/models";

const actions: ContextItemAction[] = [
  "keep",
  "compress",
  "retrieve_only",
  "exclude",
];
const actionLabels: Record<ContextItemAction, string> = {
  keep: "保留",
  compress: "压缩",
  retrieve_only: "仅检索",
  exclude: "排除",
};

export default function ContextInspectorPage() {
  const { id = "" } = useParams();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const notify = useAppStore((state) => state.notify);
  const [project, setProject] = useState<UnifiedProjectDetail | null>(null);
  const [compiled, setCompiled] = useState<CompiledContext | null>(null);
  const [snapshots, setSnapshots] = useState<ContextSnapshot[]>([]);
  const [filter, setFilter] = useState<ContextItemAction | "all">("all");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [comparisonBase, setComparisonBase] = useState<string | null>(null);
  const [snapshotDiff, setSnapshotDiff] = useState<ContextSnapshotDiff | null>(
    null,
  );

  const buildOptions = useCallback(
    (value: UnifiedProjectDetail): ContextCompileOptions => ({
      projectId: id,
      branchId: params.get("branch") || value.currentBranchId,
      sourceNodeId: null,
      targetAgent: value.defaultAgent,
      targetModel: value.defaultModel,
      tokenBudget: value.health.contextBudget,
      recentRounds: 8,
      includeToolLogs: true,
      includeGitDiff: true,
      includeFailedAttempts: true,
      includeSkills: true,
      includeMcp: true,
    }),
    [id, params],
  );
  const load = useCallback(async () => {
    setBusy(true);
    try {
      const value = await appApi.project(id);
      setProject(value);
      setCompiled(await appApi.compileContext(buildOptions(value)));
      setSnapshots(await appApi.snapshots(id));
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }, [id, buildOptions]);
  useEffect(() => {
    void load();
  }, [load]);
  const visible = useMemo(
    () =>
      compiled?.items.filter(
        (item) =>
          (filter === "all" || item.action === filter) &&
          `${item.content} ${item.reason}`
            .toLowerCase()
            .includes(query.toLowerCase()),
      ) ?? [],
    [compiled, filter, query],
  );

  async function overrideItem(item: ContextItem, patch: Partial<ContextItem>) {
    if (!project) return;
    await appApi.setContextItemOverride({
      projectId: id,
      branchId: params.get("branch") || project.currentBranchId,
      sourceNodeId: item.sourceNodeId,
      contentHash: item.contentHash,
      action: patch.action ?? item.action,
      priority: patch.priority ?? item.priority,
      pinned: patch.pinned ?? item.pinned,
      stale: patch.stale ?? item.stale,
      incorrect: patch.incorrect ?? item.incorrect,
      permanent: patch.permanent ?? item.permanent,
    });
    await load();
  }
  async function saveSnapshot() {
    if (!project) return;
    const snapshot = await appApi.saveSnapshot(buildOptions(project));
    setSnapshots((items) => [snapshot, ...items]);
    notify({
      tone: "success",
      title: "Context Snapshot 已保存",
      detail: `约 ${snapshot.estimatedTokens} tokens · ${snapshot.contentHash.slice(0, 12)}`,
    });
  }
  async function compareSnapshot(snapshotId: string) {
    if (!comparisonBase) {
      setComparisonBase(snapshotId);
      setSnapshotDiff(null);
      return;
    }
    if (comparisonBase === snapshotId) {
      setComparisonBase(null);
      setSnapshotDiff(null);
      return;
    }
    setSnapshotDiff(await appApi.diffSnapshots(comparisonBase, snapshotId));
  }
  if (error)
    return (
      <div className="page">
        <ErrorState message={error} onRetry={() => void load()} />
      </div>
    );
  if (!project || !compiled)
    return <LoadingState label="正在解释上下文压缩决策" />;

  return (
    <div className="page inspector-page">
      <button
        className="back-button"
        onClick={() => navigate(`/projects/${id}/chat`)}
      >
        <ArrowLeft size={15} />
        返回统一对话
      </button>
      <PageHeader
        eyebrow="CONTEXT INSPECTOR"
        title="上下文检查"
        description="查看每条信息为何被保留、压缩、排除或放入检索，并控制下一次续接。"
        actions={
          <>
            <button
              className="button button-secondary"
              onClick={() => void load()}
              disabled={busy}
            >
              <RefreshCw size={14} className={busy ? "animate-spin" : ""} />
              重新编译
            </button>
            <button
              className="button button-primary"
              onClick={() => void saveSnapshot()}
            >
              <Save size={14} />
              保存 Snapshot
            </button>
          </>
        }
      />
      <section className="compression-summary">
        <div className="compression-ratio">
          <span>压缩前估算</span>
          <strong>{compiled.originalEstimatedTokens.toLocaleString()}</strong>
          <i />
          <span>编译后</span>
          <strong>{compiled.estimatedTokens.toLocaleString()}</strong>
          <small>
            Token 为字符规则估算，不是模型计费结果 · SHA-256{" "}
            {compiled.contentHash.slice(0, 12)}
          </small>
        </div>
        {actions.map((action) => (
          <button
            key={action}
            onClick={() => setFilter(action)}
            className={filter === action ? "active" : ""}
          >
            <span className={`action-dot ${action}`} />
            <strong>
              {compiled.items.filter((item) => item.action === action).length}
            </strong>
            <small>{actionLabels[action]}</small>
          </button>
        ))}
      </section>
      <div className="inspector-layout">
        <main>
          <div className="toolbar">
            <label className="search-field">
              <Search size={14} />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="搜索上下文内容或处理原因"
              />
            </label>
            <button
              className="button button-secondary"
              onClick={() => setFilter("all")}
            >
              <Filter size={14} />
              全部 {compiled.items.length}
            </button>
          </div>
          <div className="context-item-list">
            {visible.map((item) => (
              <article
                key={item.id}
                className={`context-item action-${item.action}`}
              >
                <div className="context-action">
                  <span className={`action-dot ${item.action}`} />
                  <strong>{actionLabels[item.action]}</strong>
                  <small>{item.category}</small>
                </div>
                <div className="context-copy">
                  <p>{item.content}</p>
                  <div>
                    <Badge
                      tone={
                        item.action === "keep"
                          ? "success"
                          : item.action === "compress"
                            ? "warning"
                            : "neutral"
                      }
                    >
                      {item.reason}
                    </Badge>
                    <code>
                      ~{item.estimatedTokens} tokens · P{item.priority}
                    </code>
                    {item.permanent && <Badge tone="signal">永久</Badge>}
                  </div>
                  <div className="context-action-picker">
                    {actions.map((action) => (
                      <button
                        key={action}
                        className={item.action === action ? "active" : ""}
                        onClick={() => void overrideItem(item, { action })}
                      >
                        {actionLabels[action]}
                      </button>
                    ))}
                  </div>
                </div>
                <div className="context-controls">
                  <button
                    title="固定为永久上下文"
                    onClick={() =>
                      void overrideItem(item, {
                        pinned: true,
                        permanent: true,
                        priority: 100,
                      })
                    }
                  >
                    <Pin size={13} />
                    固定
                  </button>
                  <button
                    title="标记为过期"
                    onClick={() =>
                      void overrideItem(item, {
                        stale: true,
                        action: "retrieve_only",
                      })
                    }
                  >
                    <Unlink size={13} />
                    过期
                  </button>
                  <button
                    title="标记为错误"
                    onClick={() =>
                      void overrideItem(item, {
                        incorrect: true,
                        action: "exclude",
                      })
                    }
                  >
                    <XCircle size={13} />
                    错误
                  </button>
                  <button
                    title="从下次上下文排除"
                    onClick={() =>
                      void overrideItem(item, {
                        action: "exclude",
                        priority: 0,
                      })
                    }
                  >
                    <Ban size={13} />
                    排除
                  </button>
                </div>
              </article>
            ))}
          </div>
        </main>
        <aside>
          <section>
            <p className="eyebrow">CONFLICTS</p>
            <h2>冲突信息</h2>
            {compiled.conflicts.length ? (
              compiled.conflicts.map((conflict) => (
                <p className="conflict" key={conflict}>
                  <ShieldAlert size={13} />
                  {conflict}
                </p>
              ))
            ) : (
              <p className="success-text">
                <Check size={13} />
                未发现显式冲突标记
              </p>
            )}
          </section>
          <section>
            <p className="eyebrow">SNAPSHOTS</p>
            <h2>历史快照</h2>
            <p className="muted-copy">依次选择两个快照可查看差异。</p>
            {snapshotDiff && (
              <div className="snapshot-diff">
                <strong>
                  {snapshotDiff.tokenDelta >= 0 ? "+" : ""}
                  {snapshotDiff.tokenDelta} tokens
                </strong>
                <span>
                  新增 {snapshotDiff.added.length} · 删除{" "}
                  {snapshotDiff.removed.length} · 变化{" "}
                  {snapshotDiff.changed.length}
                </span>
              </div>
            )}
            {snapshots.length ? (
              snapshots.map((snapshot) => (
                <button
                  className={`snapshot-row ${comparisonBase === snapshot.id ? "active" : ""}`}
                  key={snapshot.id}
                  onClick={() => void compareSnapshot(snapshot.id)}
                >
                  <Database size={14} />
                  <span>
                    <strong>
                      {new Date(snapshot.generatedAt).toLocaleString("zh-CN")}
                    </strong>
                    <small>
                      {snapshot.estimatedTokens.toLocaleString()} /{" "}
                      {snapshot.tokenBudget.toLocaleString()} tokens
                    </small>
                  </span>
                  <Eye size={13} />
                </button>
              ))
            ) : (
              <p className="muted-copy">还没有已保存快照</p>
            )}
          </section>
        </aside>
      </div>
    </div>
  );
}
