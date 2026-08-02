import { AlertTriangle, ArrowLeft, Braces, FileCode2, GitBranch, Sparkles, Terminal, UserRound, Wrench } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { appApi } from "../api/bridge";
import { Badge, ErrorState, LoadingState, PageHeader, PathText } from "../components/ui";
import { getAgentLabel } from "../config/agents";
import type { SessionDetail } from "../types/models";

const tabs = ["Overview", "Messages", "Tool Calls", "Files", "Git State", "Raw Data"] as const;
type Tab = (typeof tabs)[number];

export default function SessionDetailPage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const [session, setSession] = useState<SessionDetail | null>(null);
  const [tab, setTab] = useState<Tab>("Overview");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    appApi.session(id).then((value) => current && setSession(value)).catch((reason: unknown) => current && setError(reason instanceof Error ? reason.message : String(reason)));
    return () => { current = false; };
  }, [id]);

  if (error) return <div className="page"><ErrorState message={error} onRetry={() => navigate("/sessions")} /></div>;
  if (!session) return <LoadingState label="正在解析会话详情与 Git 状态" />;

  return (
    <div className="page detail-page">
      <button className="back-button" onClick={() => navigate("/sessions")}><ArrowLeft size={15} />返回会话</button>
      <PageHeader eyebrow={`${getAgentLabel(session.agent)} / ${session.id}`} title={session.title} description={session.goalSummary || "此会话没有可提取的用户目标摘要。"} actions={<button className="button button-primary" onClick={() => navigate(`/sessions?source=${encodeURIComponent(session.id)}`)}><Sparkles size={15} />绑定并 Fresh Continue</button>} />
      <div className="detail-meta">
        <div><span>工作目录</span><PathText value={session.workingDirectory} /></div><div><span>Git 仓库</span><PathText value={session.gitRepository} /></div><div><span>消息</span><strong>{session.messageCount}</strong></div><div><span>工具调用</span><strong>{session.toolCallCount}</strong></div>
      </div>
      {session.parseWarning && <div className="inline-warning"><AlertTriangle size={16} /><span><strong>部分内容未解析</strong>{session.parseWarning}</span></div>}
      <div className="tabs" role="tablist" aria-label="会话详情视图">
        {tabs.map((item) => <button key={item} role="tab" aria-selected={tab === item} className={tab === item ? "active" : ""} onClick={() => setTab(item)}>{item}</button>)}
      </div>
      <section className="tab-content" role="tabpanel">
        {tab === "Overview" && <Overview session={session} />}
        {tab === "Messages" && <Messages session={session} />}
        {tab === "Tool Calls" && <ToolCalls session={session} />}
        {tab === "Files" && <Files session={session} />}
        {tab === "Git State" && <GitStatePanel session={session} />}
        {tab === "Raw Data" && <pre className="json-view">{JSON.stringify(session.rawData, null, 2)}</pre>}
      </section>
    </div>
  );
}

function Overview({ session }: { session: SessionDetail }) {
  return <div className="overview-grid">
    <section><p className="eyebrow">EXTRACTED GOAL</p><h2>目标与状态</h2><p className="prose-text">{session.goalSummary || "没有识别出明确的用户目标。"}</p><dl className="definition-list"><div><dt>创建时间</dt><dd>{new Date(session.createdAt).toLocaleString("zh-CN")}</dd></div><div><dt>最后更新</dt><dd>{new Date(session.updatedAt).toLocaleString("zh-CN")}</dd></div><div><dt>可封装</dt><dd>{session.canPackage ? "是" : "否"}</dd></div></dl></section>
    <section><p className="eyebrow">EXECUTION SIGNALS</p><h2>执行痕迹</h2><div className="signal-list"><span><Terminal size={15} />{session.commands.length} 条命令</span><span><FileCode2 size={15} />{session.changedFiles.length} 个文件</span><span className={session.failedSteps.length ? "danger-text" : ""}><AlertTriangle size={15} />{session.failedSteps.length} 个失败步骤</span><span><GitBranch size={15} />{session.gitState?.isRepository ? session.gitState.branch ?? "Git 仓库" : "非 Git 仓库"}</span></div></section>
    {session.failedSteps.length > 0 && <section className="full-span"><p className="eyebrow">FAILED STEPS</p><h2>不要重复的尝试</h2><ul className="plain-list danger-list">{session.failedSteps.map((step, index) => <li key={`${step}-${index}`}>{step}</li>)}</ul></section>}
  </div>;
}

function Messages({ session }: { session: SessionDetail }) {
  return <div className="timeline">{session.messages.map((message) => <article key={message.id} className="timeline-item"><div className={`role-mark role-${message.role}`}>{message.role === "user" ? <UserRound size={14} /> : <Braces size={14} />}</div><div><header><Badge tone={message.role === "user" ? "signal" : "neutral"}>{message.role}</Badge><time>{message.timestamp ? new Date(message.timestamp).toLocaleString("zh-CN") : "无时间戳"}</time></header><p>{message.content}</p></div></article>)}</div>;
}

function ToolCalls({ session }: { session: SessionDetail }) {
  return <div className="tool-call-list">{session.toolCalls.map((call) => <article key={call.id}><header><Wrench size={15} /><strong>{call.name}</strong><Badge tone={call.status === "failed" ? "danger" : call.status === "success" ? "success" : "neutral"}>{call.status}</Badge></header><pre>{call.arguments}</pre>{call.output && <details><summary>查看结果</summary><pre>{call.output}</pre></details>}</article>)}</div>;
}

function Files({ session }: { session: SessionDetail }) {
  return <div className="file-list">{session.changedFiles.length ? session.changedFiles.map((file) => <div key={file}><FileCode2 size={15} /><PathText value={file} /></div>) : <p className="text-muted">会话记录中未提取到文件改动。</p>}</div>;
}

function GitStatePanel({ session }: { session: SessionDetail }) {
  const git = session.gitState;
  if (!git?.isRepository) return <div className="state-panel"><GitBranch size={19} /><div><strong>当前目录不是 Git 仓库</strong><p>{git?.error || "未读取 Git 状态。"}</p></div></div>;
  return <div className="git-panel"><div className="git-summary"><div><span>分支</span><strong>{git.branch}</strong></div><div><span>HEAD</span><code>{git.head}</code></div><div><span>已修改</span><strong>{git.modified.length}</strong></div><div><span>已暂存</span><strong>{git.staged.length}</strong></div><div><span>未跟踪</span><strong>{git.untracked.length}</strong></div></div><h2>Working tree diff</h2><pre className="diff-view">{git.workingTreeDiff || "（无未提交补丁）"}</pre><h2>Staged diff</h2><pre className="diff-view">{git.stagedDiff || "（无暂存补丁）"}</pre></div>;
}
