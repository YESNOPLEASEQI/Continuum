import { Archive, ArrowRight, Boxes, Clock3, FolderOpen, PackagePlus, RadioTower, RefreshCw, Waypoints } from "lucide-react";
import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { Badge, EmptyState, ErrorState, LoadingState, PageHeader, PathText } from "../components/ui";
import { getAgentLabel } from "../config/agents";
import { useAppStore } from "../store/appStore";
import { useMajorSessionScan } from "../motion/useMajorSessionScan";

function formatTime(value: string | null): string {
  if (!value) return "尚未扫描";
  return new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}

export default function DashboardPage() {
  const navigate = useNavigate();
  const { dashboard, loading, error, scanning, loadDashboard } = useAppStore();
  const scanSessions = useMajorSessionScan();
  useEffect(() => { if (!dashboard) void loadDashboard(); }, [dashboard, loadDashboard]);

  if (loading && !dashboard) return <LoadingState label="正在初始化本地工作区" />;
  if (error && !dashboard) return <ErrorState message={error} onRetry={() => void loadDashboard()} />;

  return (
    <div className="page dashboard-page">
      <PageHeader eyebrow="LOCAL HANDOFF CONTROL" title="任务接力总览" description="从本机 Agent 会话中提取可验证、可迁移的工作状态。" />

      <section className="metric-strip" aria-label="工作区指标">
        <div className="primary-metric"><span>已发现会话</span><strong>{dashboard?.sessionCount ?? 0}</strong><small>SQLite 索引</small></div>
        <div className="metric"><Boxes size={17} /><span>任务包</span><strong>{dashboard?.packageCount ?? 0}</strong></div>
        <div className="metric"><Archive size={17} /><span>已导入</span><strong>{dashboard?.importedPackageCount ?? 0}</strong></div>
        <div className="metric metric-wide"><Waypoints size={17} /><span>检测到的 Agent</span><strong>{dashboard?.detectedAgents.length ? dashboard.detectedAgents.map(getAgentLabel).join(" · ") : "等待扫描"}</strong></div>
        <div className="metric metric-wide"><Clock3 size={17} /><span>最近扫描</span><strong>{formatTime(dashboard?.lastScanAt ?? null)}</strong></div>
      </section>

      <div className="dashboard-grid">
        <section className="content-section">
          <div className="section-heading"><div><p className="eyebrow">RECENT PACKAGES</p><h2>最近任务包</h2></div><button className="text-button" onClick={() => navigate("/packages")}>查看全部 <ArrowRight size={14} /></button></div>
          {dashboard?.recentPackages.length ? (
            <div className="package-rows">
              {dashboard.recentPackages.map((item) => (
                <button className="package-row" key={item.id} onClick={() => navigate(`/packages/${item.id}`)}>
                  <span className="file-glyph">AP</span><span className="row-main"><strong>{item.title}</strong><PathText value={item.projectPath} /></span>
                  <Badge tone={item.integrity === "valid" ? "success" : "warning"}>{item.integrity === "valid" ? "完整" : "需检查"}</Badge>
                  <span className="row-time">{formatTime(item.createdAt)}</span><ArrowRight size={15} />
                </button>
              ))}
            </div>
          ) : <EmptyState icon={<Boxes size={23} />} title="还没有任务包" detail="扫描会话后，从一段真实的 Codex 会话创建第一个接力包。" action={<button className="button button-primary" onClick={() => navigate("/packages/new")}>创建任务包</button>} />}
        </section>

        <aside className="action-panel">
          <p className="eyebrow">QUICK ACTIONS</p><h2>继续工作</h2>
          <button className="action-row primary" onClick={() => void scanSessions()} disabled={scanning}><RefreshCw size={18} className={scanning ? "animate-spin" : ""} /><span><strong>{scanning ? "正在扫描" : "扫描本地会话"}</strong><small>读取已配置的 Codex JSONL</small></span><kbd>R</kbd></button>
          <button className="action-row" onClick={() => navigate("/packages/new")}><PackagePlus size={18} /><span><strong>创建任务包</strong><small>从已索引会话提取状态</small></span></button>
          <button className="action-row" onClick={() => navigate("/packages")}><FolderOpen size={18} /><span><strong>导入或打开</strong><small>验证本地 .agentpack.zip</small></span></button>
          <button className="action-row" onClick={() => navigate("/sessions")}><RadioTower size={18} /><span><strong>检查会话</strong><small>消息、工具与 Git 状态</small></span></button>
          <div className="local-boundary"><span className="status-led ok" /><div><strong>本地边界有效</strong><p>未配置任何远程服务；所有数据留在设备上。</p></div></div>
        </aside>
      </div>
      <p className="database-footnote">数据库 <PathText value={dashboard?.databasePath} /></p>
    </div>
  );
}
