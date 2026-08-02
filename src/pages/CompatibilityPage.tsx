import { AlertTriangle, ArrowRight, Check, MinusCircle, ShieldCheck, Waypoints, Wrench } from "lucide-react";
import { useEffect, useMemo, useState, type CSSProperties } from "react";
import { PageHeader } from "../components/ui";
import { agentCapabilities, calculateCompatibility } from "../config/agents";
import { useAppStore } from "../store/appStore";
import type { AgentKind } from "../types/models";

export default function CompatibilityPage() {
  const { packages, loadPackages } = useAppStore();
  const [source, setSource] = useState<AgentKind>("codex");
  const [target, setTarget] = useState<AgentKind>("claude");
  const [packageId, setPackageId] = useState("");
  useEffect(() => { void loadPackages(); }, [loadPackages]);
  const result = useMemo(() => calculateCompatibility(source, target), [source, target]);
  const selectedPackage = packages.find((item) => item.id === packageId);

  return <div className="page compatibility-page">
    <PageHeader eyebrow="CAPABILITY MAP" title="Agent 兼容性" description="根据结构化能力表评估任务上下文能否安全迁移；不会调用目标 Agent。" />
    <section className="compatibility-selector">
      <label><span>来源 Agent</span><select value={source} onChange={(event) => setSource(event.target.value as AgentKind)}>{agentCapabilities.map((agent) => <option value={agent.id} key={agent.id}>{agent.label}</option>)}</select></label><ArrowRight size={19} />
      <label><span>目标 Agent</span><select value={target} onChange={(event) => setTarget(event.target.value as AgentKind)}>{agentCapabilities.map((agent) => <option value={agent.id} key={agent.id}>{agent.label}</option>)}</select></label>
      <label className="package-select"><span>当前任务包（可选）</span><select value={packageId} onChange={(event) => setPackageId(event.target.value)}><option value="">通用能力评估</option>{packages.map((item) => <option value={item.id} key={item.id}>{item.title}</option>)}</select></label>
    </section>
    <section className="compat-score"><div className="score-dial" style={{ "--score": `${result.score * 3.6}deg` } as CSSProperties}><span>{result.score}</span><small>/ 100</small></div><div><p className="eyebrow">COMPATIBILITY SCORE</p><h2>{result.score >= 80 ? "高兼容，可直接恢复" : result.score >= 60 ? "可迁移，需要人工确认" : "迁移风险较高"}</h2><p>{selectedPackage ? `已结合「${selectedPackage.title}」的目标 Agent 配置。` : "当前为通用能力配置评估。"}</p></div></section>
    <div className="compat-grid">
      <section><p className="eyebrow">SOURCE CAPABILITIES</p><h2>{result.source.label}</h2><ul className="cap-list">{result.source.capabilities.map((item) => <li key={item}><Check size={14} />{item}</li>)}</ul></section>
      <section><p className="eyebrow">TARGET CAPABILITIES</p><h2>{result.target.label}</h2><ul className="cap-list">{result.target.capabilities.map((item) => <li key={item}><Check size={14} />{item}</li>)}</ul></section>
      <section><p className="eyebrow">MISSING TOOLS</p><h2>缺少的工具</h2>{result.missingTools.length ? <ul className="cap-list warning-list">{result.missingTools.map((item) => <li key={item}><Wrench size={14} />{item}</li>)}</ul> : <p className="success-text"><ShieldCheck size={15} />没有检测到工具缺口。</p>}</section>
      <section><p className="eyebrow">NON-PORTABLE</p><h2>不可直接迁移</h2><ul className="cap-list warning-list">{result.nonPortable.map((item) => <li key={item}><MinusCircle size={14} />{item}</li>)}</ul></section>
      <section className="full-span recommendation"><AlertTriangle size={19} /><div><p className="eyebrow">RECOVERY ADVICE</p><h2>恢复建议</h2><p>先让目标 Agent 阅读 Goal、State、Decisions 与 Constraints，再核对当前文件和 Git 状态。对缺失工具的步骤应改为能力等价的操作，且不要自动执行命令日志。</p></div></section>
    </div>
  </div>;
}
