import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, ArrowLeft, CheckCircle2, Clipboard, Download, FileJson2, FolderOutput, RefreshCw, ShieldAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { appApi } from "../api/bridge";
import { Badge, ErrorState, LoadingState, PageHeader, PathText } from "../components/ui";
import { getAgentLabel } from "../config/agents";
import { useAppStore } from "../store/appStore";
import type { PackageDetail, ValidationReport } from "../types/models";

export default function PackageDetailPage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const notify = useAppStore((state) => state.notify);
  const [detail, setDetail] = useState<PackageDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [jsonMode, setJsonMode] = useState(false);
  const [validation, setValidation] = useState<ValidationReport | null>(null);
  const [validating, setValidating] = useState(false);

  useEffect(() => { appApi.package(id).then(setDetail).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : String(reason))); }, [id]);
  async function validate() { setValidating(true); try { const result = await appApi.validatePackage(id); setValidation(result); notify({ tone: result.valid ? "success" : "error", title: result.valid ? "任务包校验通过" : "任务包存在问题", detail: `${result.issues.length} 项结果` }); } finally { setValidating(false); } }
  async function exportFolder() { const destination = await open({ directory: true, multiple: false }); if (!destination || Array.isArray(destination)) return; const result = await appApi.exportFolder(id, destination); notify({ tone: "success", title: "文件夹已导出", detail: result }); }
  async function exportZip() { const result = await appApi.exportZip(id); notify({ tone: "success", title: "Zip 已导出", detail: result }); }

  if (error) return <div className="page"><ErrorState message={error} onRetry={() => navigate("/packages")} /></div>;
  if (!detail) return <LoadingState label="正在读取并验证任务包" />;

  const structured = { manifest: detail.manifest, goal: detail.goal, state: detail.state, decisions: detail.decisions, failedAttempts: detail.failedAttempts, constraints: detail.constraints, capabilities: detail.capabilities, nextActions: detail.nextActions, provenance: detail.provenance, securityFindings: detail.securityFindings };
  return <div className="page detail-page">
    <button className="back-button" onClick={() => navigate("/packages")}><ArrowLeft size={15} />返回任务包</button>
    <PageHeader eyebrow={`AGENTPACK / ${detail.manifest.packageId}`} title={detail.title} description={`${getAgentLabel(detail.sourceAgent)} → ${getAgentLabel(detail.targetAgent)} · ${detail.schemaVersion}`} actions={<><button className="button button-secondary" onClick={() => setJsonMode((value) => !value)}><FileJson2 size={15} />{jsonMode ? "可读模式" : "JSON 模式"}</button><button className="button button-primary" onClick={() => void validate()} disabled={validating}><RefreshCw size={15} className={validating ? "animate-spin" : ""} />完整性检查</button></>} />
    <div className="package-detail-strip"><div><span>创建时间</span><strong>{new Date(detail.createdAt).toLocaleString("zh-CN")}</strong></div><div><span>项目路径</span><PathText value={detail.projectPath} /></div><div><span>完整性</span><Badge tone={detail.integrity === "valid" ? "success" : "warning"}>{detail.integrity}</Badge></div><div><span>安全警告</span><strong className={detail.securityWarningCount ? "danger-text" : ""}>{detail.securityWarningCount}</strong></div></div>
    <div className="detail-actionbar"><button className="button button-secondary" onClick={async () => { await navigator.clipboard.writeText(detail.resumePrompt); notify({ tone: "success", title: "恢复提示词已复制" }); }}><Clipboard size={15} />复制恢复提示词</button><button className="button button-secondary" onClick={() => void exportFolder()}><FolderOutput size={15} />导出文件夹</button><button className="button button-secondary" onClick={() => void exportZip()}><Download size={15} />导出 .agentpack.zip</button><button className="text-button" onClick={async () => { await appApi.markResumed(id); notify({ tone: "success", title: "已标记为恢复" }); }}><CheckCircle2 size={14} />标记为已恢复</button></div>
    {validation && <div className={`validation-banner ${validation.valid ? "valid" : "invalid"}`}>{validation.valid ? <CheckCircle2 size={18} /> : <AlertTriangle size={18} />}<div><strong>{validation.valid ? "结构、哈希与路径检查通过" : "任务包需要处理"}</strong>{validation.issues.map((issue) => <p key={`${issue.code}-${issue.path}`}>{issue.code}: {issue.message}</p>)}</div></div>}
    {jsonMode ? <pre className="json-view">{JSON.stringify(structured, null, 2)}</pre> : <ReadablePackage detail={detail} />}
  </div>;
}

function Section({ label, title, value }: { label: string; title: string; value: unknown }) {
  const display = typeof value === "string" ? value : JSON.stringify(value, null, 2);
  return <section className="package-section"><p className="eyebrow">{label}</p><h2>{title}</h2><pre>{display || "—"}</pre></section>;
}

function ReadablePackage({ detail }: { detail: PackageDetail }) {
  return <div className="readable-package">
    <Section label="GOAL" title="任务目标" value={detail.goal} /><Section label="CURRENT STATE" title="当前状态" value={detail.state} /><Section label="NEXT ACTIONS" title="下一步操作" value={detail.nextActions} /><Section label="DECISIONS" title="关键决策" value={detail.decisions} /><Section label="FAILED ATTEMPTS" title="失败尝试" value={detail.failedAttempts} /><Section label="CONSTRAINTS" title="约束" value={detail.constraints} /><Section label="CAPABILITIES" title="所需能力" value={detail.capabilities} /><Section label="WORKSPACE / PROVENANCE" title="工作区与来源" value={detail.provenance} />
    <section className="package-section full-span"><p className="eyebrow">SECURITY WARNINGS</p><h2>安全检查</h2>{detail.securityFindings.length ? <div className="finding-list">{detail.securityFindings.map((finding, index) => <div key={`${finding.findingType}-${index}`}><ShieldAlert size={15} /><strong>{finding.findingType}</strong><PathText value={finding.sourceFile} /><code>{finding.fieldPath}</code></div>)}</div> : <p className="success-text"><CheckCircle2 size={15} />没有发现待处理的敏感信息。</p>}</section>
    <section className="package-section full-span resume-prompt"><p className="eyebrow">RESUME PROMPT</p><h2>恢复提示词</h2><pre>{detail.resumePrompt}</pre></section>
  </div>;
}
