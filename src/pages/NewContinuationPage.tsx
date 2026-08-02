import {
  AlertTriangle,
  ArrowLeft,
  Check,
  Clipboard,
  FileText,
  LoaderCircle,
  Play,
  RadioTower,
  RotateCcw,
  Sparkles,
  Terminal,
  Trash2,
  Waypoints,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { appApi } from "../api/bridge";
import {
  Badge,
  ErrorState,
  LoadingState,
  PageHeader,
  PathText,
  Toggle,
} from "../components/ui";
import { useAppStore } from "../store/appStore";
import type {
  CompiledContext,
  ContextCompileOptions,
  ContinuationRecord,
  SessionSummary,
  UnifiedProjectDetail,
} from "../types/models";

const statusLabels: Record<string, string> = {
  idle: "等待开始",
  compiling_context: "正在编译上下文",
  writing_context: "正在写入上下文文件",
  preparing_launch: "已准备启动",
  launching: "正在启动 Codex",
  waiting_for_session: "等待检测新会话",
  candidate_sessions_found: "已发现候选会话",
  binding: "正在绑定新会话",
  manual_binding_required: "需要手工选择新会话",
  listening: "已绑定，监听中",
  launch_failed: "启动失败",
  detection_timeout: "检测新会话超时",
  completed: "已完成",
  cancelled: "已取消",
};

export default function NewContinuationPage() {
  const { id = "" } = useParams();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const notify = useAppStore((state) => state.notify);
  const [project, setProject] = useState<UnifiedProjectDetail | null>(null);
  const [compiled, setCompiled] = useState<CompiledContext | null>(null);
  const [record, setRecord] = useState<ContinuationRecord | null>(null);
  const [candidates, setCandidates] = useState<SessionSummary[]>([]);
  const [stage, setStage] = useState("compiling_context");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [options, setOptions] = useState<ContextCompileOptions>({
    projectId: id,
    branchId: params.get("branch") ?? "",
    sourceNodeId: params.get("node") || null,
    targetAgent:
      (params.get("target") as ContextCompileOptions["targetAgent"]) || "codex",
    targetModel: "default",
    tokenBudget: 32000,
    recentRounds: 8,
    includeToolLogs: true,
    includeGitDiff: true,
    includeFailedAttempts: true,
    includeSkills: true,
    includeMcp: true,
  });
  useEffect(() => {
    appApi
      .project(id)
      .then((value) => {
        setProject(value);
        setOptions((current) => ({
          ...current,
          branchId: current.branchId || value.currentBranchId,
          targetAgent: current.targetAgent || value.defaultAgent,
          targetModel: value.defaultModel,
          tokenBudget: value.health.contextBudget,
        }));
      })
      .catch((reason: unknown) =>
        setError(reason instanceof Error ? reason.message : String(reason)),
      );
  }, [id]);
  const preview = useCallback(async () => {
    if (!options.branchId) return;
    setBusy(true);
    setStage("compiling_context");
    try {
      setCompiled(await appApi.compileContext(options));
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }, [options]);
  useEffect(() => {
    if (project && options.branchId) void preview();
  }, [project, options.branchId]);
  useEffect(() => {
    if (
      !record ||
      ![
        "waiting_for_session",
        "candidate_sessions_found",
        "manual_binding_required",
        "listening",
      ].includes(record.status)
    )
      return;
    const timer = window.setInterval(async () => {
      try {
        const result = await appApi.pollContinuation(record.id);
        setRecord(result.continuation);
        setStage(result.continuation.status);
        setCandidates(result.candidates);
        if (result.continuation.status === "listening") {
          window.clearInterval(timer);
          notify({
            tone: "success",
            title: "Fresh Continuation 已绑定",
            detail: result.continuation.targetSessionId ?? undefined,
          });
        }
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      }
    }, 2000);
    return () => window.clearInterval(timer);
  }, [record?.id, record?.status, notify]);
  async function launch() {
    setBusy(true);
    setStage("compiling_context");
    try {
      setCompiled(await appApi.compileContext(options));
      const launched = await appApi.createContinuation(options, true);
      setRecord(launched);
      setStage(launched.status);
      if (launched.status === "launch_failed") setError(launched.warning);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }
  async function bindCandidate(sessionId: string) {
    if (!record) return;
    await appApi.bindContinuation(record.id, sessionId);
    const result = await appApi.pollContinuation(record.id);
    setRecord(result.continuation);
    setCandidates([]);
  }
  const retention = useMemo(
    () =>
      compiled
        ? {
            keep: compiled.items.filter((item) => item.action === "keep")
              .length,
            compress: compiled.items.filter(
              (item) => item.action === "compress",
            ).length,
            retrieveOnly: compiled.items.filter(
              (item) => item.action === "retrieve_only",
            ).length,
            exclude: compiled.items.filter((item) => item.action === "exclude")
              .length,
          }
        : null,
    [compiled],
  );
  if (error && !project)
    return (
      <div className="page">
        <ErrorState message={error} />
      </div>
    );
  if (!project) return <LoadingState label="正在准备 Fresh Continuation" />;
  return (
    <div className="page continuation-page">
      <button
        className="back-button"
        onClick={() => navigate(`/projects/${id}/chat`)}
      >
        <ArrowLeft size={15} />
        返回统一对话
      </button>
      <PageHeader
        eyebrow="FRESH CONTINUATION"
        title="压缩后开启干净会话"
        description="创建全新 Codex 会话，只注入规则编译后的必要上下文；不会调用 codex resume 或 codex fork。"
      />
      <div className="mode-comparison">
        <div>
          <Badge tone="neutral">Resume</Badge>
          <strong>恢复原会话</strong>
          <p>继续原有长历史，不解决上下文冗长。</p>
        </div>
        <div>
          <Badge tone="neutral">Fork</Badge>
          <strong>从原历史分叉</strong>
          <p>仍可能继承旧会话历史。</p>
        </div>
        <div className="active">
          <Badge tone="signal">PRIMARY</Badge>
          <strong>Fresh Continuation</strong>
          <p>新建干净会话，仅注入编译后的上下文。</p>
        </div>
      </div>
      <div className="continuation-grid">
        <section className="continuation-form">
          <div className="section-heading compact">
            <div>
              <p className="eyebrow">TARGET SESSION</p>
              <h2>续接目标</h2>
            </div>
          </div>
          <div className="form-grid two-cols compact-form">
            <label>
              <span>目标 Agent</span>
              <select
                value={options.targetAgent}
                onChange={(event) =>
                  setOptions({
                    ...options,
                    targetAgent: event.target
                      .value as ContextCompileOptions["targetAgent"],
                  })
                }
              >
                <option value="codex">Codex CLI · 自动上下文续接</option>
                <option value="claude">Claude Code · 仅导出（框架）</option>
              </select>
            </label>
            <label>
              <span>目标模型</span>
              <input
                value={options.targetModel}
                onChange={(event) =>
                  setOptions({ ...options, targetModel: event.target.value })
                }
              />
            </label>
            <label className="full-field">
              <span>目标工作目录</span>
              <div className="readonly-path">
                <PathText value={project.projectPath} />
              </div>
            </label>
            <label>
              <span>上下文预算</span>
              <input
                type="number"
                min={1000}
                step={1000}
                value={options.tokenBudget}
                onChange={(event) =>
                  setOptions({
                    ...options,
                    tokenBudget: Number(event.target.value),
                  })
                }
              />
            </label>
            <label>
              <span>保留最近轮数</span>
              <input
                type="number"
                min={1}
                max={50}
                value={options.recentRounds}
                onChange={(event) =>
                  setOptions({
                    ...options,
                    recentRounds: Number(event.target.value),
                  })
                }
              />
            </label>
          </div>
          <div className="toggle-stack single">
            <Toggle
              checked={options.includeToolLogs}
              onChange={(value) =>
                setOptions({ ...options, includeToolLogs: value })
              }
              label="包含工具日志"
              detail="超长输出只保留可解释摘要"
            />
            <Toggle
              checked={options.includeGitDiff}
              onChange={(value) =>
                setOptions({ ...options, includeGitDiff: value })
              }
              label="包含 Git Diff"
              detail="读取当前工作树，不执行修改"
            />
            <Toggle
              checked={options.includeFailedAttempts}
              onChange={(value) =>
                setOptions({ ...options, includeFailedAttempts: value })
              }
              label="包含失败尝试"
              detail="避免新会话重复已知错误"
            />
            <Toggle
              checked={options.includeSkills}
              onChange={(value) =>
                setOptions({ ...options, includeSkills: value })
              }
              label="包含已绑定 Skills"
              detail="只注入统一描述，不改写第三方配置"
            />
            <Toggle
              checked={options.includeMcp}
              onChange={(value) =>
                setOptions({ ...options, includeMcp: value })
              }
              label="包含 MCP 配置"
              detail="仅描述已绑定服务与兼容性"
            />
          </div>
          <button
            className="button button-secondary button-wide"
            onClick={() => void preview()}
            disabled={busy}
          >
            <Waypoints size={15} />
            重新编译预览
          </button>
        </section>
        <section className="context-preview">
          <header>
            <div>
              <p className="eyebrow">COMPILED CONTEXT</p>
              <h2>上下文预览</h2>
            </div>
            {compiled && (
              <div className="token-estimate">
                <strong>{compiled.estimatedTokens.toLocaleString()}</strong>
                <span>/ {compiled.tokenBudget.toLocaleString()} tokens</span>
              </div>
            )}
          </header>
          {busy && !compiled ? (
            <LoadingState label="正在应用确定性压缩规则" />
          ) : compiled ? (
            <>
              <div className="retention-strip">
                <span>
                  <i className="keep" />
                  保留 {retention?.keep}
                </span>
                <span>
                  <i className="compress" />
                  压缩 {retention?.compress}
                </span>
                <span>
                  <i className="retrieve_only" />
                  仅检索 {retention?.retrieveOnly}
                </span>
                <span>
                  <i className="exclude" />
                  排除 {retention?.exclude}
                </span>
              </div>
              <pre>{compiled.compiledText}</pre>
              <div className="preview-actions">
                <button
                  className="text-button"
                  onClick={async () => {
                    await navigator.clipboard.writeText(compiled.compiledText);
                    notify({ tone: "success", title: "上下文已复制" });
                  }}
                >
                  <Clipboard size={13} />
                  复制（备用）
                </button>
                <button
                  className="button button-primary fresh-launch"
                  onClick={() => void launch()}
                  disabled={busy || Boolean(record)}
                >
                  <Sparkles size={15} />
                  启动 Fresh Continuation
                </button>
              </div>
            </>
          ) : (
            <p>等待编译。</p>
          )}
        </section>
      </div>
      {(record || busy) && (
        <section className="continuation-status">
          <header>
            <div>
              <p className="eyebrow">AUTOMATIC HANDOFF</p>
              <h2>{statusLabels[stage] ?? stage}</h2>
            </div>
            {stage === "waiting_for_session" && (
              <LoaderCircle className="animate-spin text-signal" size={20} />
            )}
            {stage === "listening" && (
              <Badge tone="success">
                <Check size={12} />
                监听中
              </Badge>
            )}
            {stage === "launch_failed" && (
              <Badge tone="danger">
                <AlertTriangle size={12} />
                启动失败
              </Badge>
            )}
          </header>
          <div className="continuation-steps">
            {[
              "正在编译上下文",
              "正在写入上下文文件",
              "正在启动 Codex",
              "等待检测新会话",
              "已绑定新会话",
              "监听中",
            ].map((label, index) => {
              const activeIndex =
                stage === "compiling_context"
                  ? 0
                  : stage === "writing_context"
                    ? 1
                    : stage === "launching"
                      ? 2
                      : stage === "waiting_for_session" ||
                          stage === "candidate_sessions_found" ||
                          stage === "manual_binding_required" ||
                          stage === "detection_timeout"
                        ? 3
                        : stage === "binding"
                          ? 4
                          : stage === "listening"
                            ? 5
                            : 2;
              return (
                <div
                  key={label}
                  className={
                    index < activeIndex
                      ? "done"
                      : index === activeIndex
                        ? "active"
                        : ""
                  }
                >
                  <span>
                    {index < activeIndex ? <Check size={11} /> : index + 1}
                  </span>
                  <strong>{label}</strong>
                </div>
              );
            })}
          </div>
          {record && (
            <div className="continuation-facts">
              <div>
                <span>Continuation ID</span>
                <code>{record.marker}</code>
              </div>
              <div>
                <span>上下文文件</span>
                <PathText value={record.bootstrapFile} />
              </div>
              <div>
                <span>SHA-256</span>
                <code>{record.contextHash}</code>
              </div>
              <div>
                <span>进程 ID</span>
                <code>{record.processId ?? "未启动"}</code>
              </div>
              <div>
                <span>目标 Session</span>
                <code>{record.targetSessionId ?? "等待识别"}</code>
              </div>
            </div>
          )}
          {record?.warning && (
            <div className="inline-warning">
              <AlertTriangle size={15} />
              {record.warning}
            </div>
          )}
          {candidates.length > 0 && (
            <div className="candidate-list">
              <h3>请选择匹配的新 Codex 会话</h3>
              {candidates.map((candidate) => (
                <button
                  key={candidate.id}
                  onClick={() => void bindCandidate(candidate.id)}
                >
                  <RadioTower size={15} />
                  <span>
                    <strong>{candidate.title}</strong>
                    <PathText value={candidate.workingDirectory} />
                  </span>
                  <code>{candidate.id}</code>
                  <ArrowRightIcon />
                </button>
              ))}
            </div>
          )}
          {record?.status === "listening" && (
            <div className="bound-actions">
              <button
                className="button button-primary"
                onClick={() => navigate(`/projects/${id}/chat`)}
              >
                <RadioTower size={15} />
                返回统一时间线
              </button>
              <button
                className="button button-secondary"
                onClick={async () => {
                  await navigator.clipboard.writeText(record.launchCommand);
                  notify({ tone: "success", title: "启动命令已复制" });
                }}
              >
                <Terminal size={15} />
                复制启动命令
              </button>
              {record.bootstrapFile && (
                <button
                  className="button button-secondary"
                  onClick={async () => {
                    const cleaned = await appApi.cleanupContinuationContext(
                      record.id,
                    );
                    setRecord(cleaned);
                    notify({ tone: "success", title: "临时上下文文件已清理" });
                  }}
                >
                  <Trash2 size={14} />
                  清理上下文文件
                </button>
              )}
            </div>
          )}
          {record &&
            [
              "launch_failed",
              "detection_timeout",
              "manual_binding_required",
            ].includes(record.status) && (
              <div className="bound-actions">
                <button
                  className="button button-primary"
                  onClick={async () => {
                    const retried = await appApi.retryContinuation(record.id);
                    setRecord(retried);
                    setStage(retried.status);
                    setError(null);
                  }}
                >
                  <RotateCcw size={14} />
                  重试
                </button>
                <button
                  className="button button-secondary"
                  onClick={async () => {
                    const cancelled = await appApi.cancelContinuation(
                      record.id,
                    );
                    setRecord(cancelled);
                    setStage(cancelled.status);
                  }}
                >
                  <XCircle size={14} />
                  取消续接
                </button>
              </div>
            )}
        </section>
      )}
    </div>
  );
}

function ArrowRightIcon() {
  return <Play size={14} />;
}
