import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { AlertTriangle, Boxes, CheckCircle2, Copy, Download, ExternalLink, PackageOpen, PackagePlus, ShieldAlert, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { appApi } from "../api/bridge";
import { Badge, ConfirmDialog, EmptyState, ErrorState, LoadingState, PageHeader, PathText } from "../components/ui";
import { getAgentLabel } from "../config/agents";
import { useAppStore } from "../store/appStore";
import type { PackageSummary } from "../types/models";

export default function PackagesPage() {
  const navigate = useNavigate();
  const { packages, loading, error, loadPackages, notify } = useAppStore();
  const [deleting, setDeleting] = useState<PackageSummary | null>(null);
  useEffect(() => { void loadPackages(); }, [loadPackages]);

  async function importPackage() {
    const selected = await open({ multiple: false, filters: [{ name: "AgentPack", extensions: ["zip", "agentpack.zip"] }, { name: "Folder", extensions: ["*"] }] });
    if (!selected || Array.isArray(selected)) return;
    try { const item = await appApi.importPackage(selected); notify({ tone: "success", title: "任务包已导入并校验", detail: item.title }); await loadPackages(); }
    catch (reason) { notify({ tone: "error", title: "导入失败", detail: reason instanceof Error ? reason.message : String(reason) }); }
  }

  async function removePackage() {
    if (!deleting) return;
    try { await appApi.deletePackage(deleting.id); notify({ tone: "success", title: "任务包已删除" }); setDeleting(null); await loadPackages(); }
    catch (reason) { notify({ tone: "error", title: "删除失败", detail: reason instanceof Error ? reason.message : String(reason) }); }
  }

  return <div className="page">
    <PageHeader eyebrow="PACKAGE LIBRARY" title="任务包" description="创建、导入和验证存放在本机磁盘上的 AgentPack。" actions={<><button className="button button-secondary" onClick={() => void importPackage()}><PackageOpen size={15} />导入</button><button className="button button-primary" onClick={() => navigate("/packages/new")}><PackagePlus size={15} />创建</button></>} />
    {loading && !packages.length ? <LoadingState label="正在读取任务包索引" /> : error ? <ErrorState message={error} onRetry={() => void loadPackages()} /> : !packages.length ? <EmptyState icon={<Boxes size={24} />} title="任务包库是空的" detail="从一个已扫描会话创建任务包，或导入现有的 .agentpack.zip。" action={<div className="inline-actions"><button className="button button-primary" onClick={() => navigate("/packages/new")}>创建任务包</button><button className="button button-secondary" onClick={() => void importPackage()}>导入文件</button></div>} /> : (
      <div className="package-library">{packages.map((item) => <article className="package-item" key={item.id}>
        <div className="package-icon"><span>AP</span>{item.imported && <small>IN</small>}</div>
        <div className="package-copy"><button onClick={() => navigate(`/packages/${item.id}`)}><strong>{item.title}</strong></button><PathText value={item.projectPath} /><div className="package-tags"><Badge tone="signal">{getAgentLabel(item.sourceAgent)} → {getAgentLabel(item.targetAgent)}</Badge><Badge tone={item.integrity === "valid" ? "success" : item.integrity === "invalid" ? "danger" : "warning"}>{item.integrity === "valid" ? <><CheckCircle2 size={11} />完整</> : <><AlertTriangle size={11} />{item.integrity}</>}</Badge>{item.hasPatch && <Badge>Git patch</Badge>}{item.securityWarningCount > 0 && <Badge tone="danger"><ShieldAlert size={11} />{item.securityWarningCount} 项警告</Badge>}</div></div>
        <div className="package-date"><span>{new Date(item.createdAt).toLocaleDateString("zh-CN")}</span><code>{item.schemaVersion}</code></div>
        <div className="row-menu"><button className="icon-button" title="复制恢复提示词" aria-label={`复制 ${item.title} 的恢复提示词`} onClick={async () => { const detail = await appApi.package(item.id); await navigator.clipboard.writeText(detail.resumePrompt); notify({ tone: "success", title: "恢复提示词已复制" }); }}><Copy size={15} /></button><button className="icon-button" title="导出 zip" aria-label={`导出 ${item.title}`} onClick={async () => { const path = await appApi.exportZip(item.id); notify({ tone: "success", title: "Zip 已导出", detail: path }); }}><Download size={15} /></button><button className="icon-button" title="在资源管理器中打开" aria-label={`打开 ${item.title} 的目录`} onClick={() => void openPath(item.packagePath)}><ExternalLink size={15} /></button><button className="icon-button danger" title="删除" aria-label={`删除 ${item.title}`} onClick={() => setDeleting(item)}><Trash2 size={15} /></button></div>
      </article>)}</div>
    )}
    <ConfirmDialog open={Boolean(deleting)} title="删除这个任务包？" description="数据库记录和 AgentPack 文件夹将同时删除。该操作无法撤销。" confirmLabel="删除任务包" destructive onCancel={() => setDeleting(null)} onConfirm={() => void removePackage()} />
  </div>;
}
