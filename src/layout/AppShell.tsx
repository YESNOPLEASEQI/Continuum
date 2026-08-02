import {
  FolderKanban,
  Link2,
  Stethoscope,
  RadioTower,
  RefreshCw,
  Settings,
  Search,
  Sparkles,
  TerminalSquare,
  X,
} from "lucide-react";
import { useEffect, useRef } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { useAppStore } from "../store/appStore";
import { ToastIcon } from "../components/ui";
import { appApi } from "../api/bridge";

const navigation = [
  { to: "/diagnostics", label: "Diagnostics", icon: Stethoscope },
  { to: "/search", label: "搜索与命令", icon: Search },
  { to: "/profiles", label: "Codex Profiles", icon: TerminalSquare },
  { to: "/projects", label: "统一项目", icon: FolderKanban },
  { to: "/sessions", label: "会话", icon: RadioTower },
  { to: "/configurations", label: "Skills 与配置", icon: Sparkles },
  { to: "/settings", label: "设置", icon: Settings },
];

export default function AppShell() {
  const location = useLocation();
  const navigate = useNavigate();
  const mainRef = useRef<HTMLElement>(null);
  const { dashboard, projects, scanning, error, toasts, watcherErrorCount, scanSessions, loadDashboard, loadProjects, pollChanges, dismissToast } = useAppStore();

  useEffect(() => { void loadDashboard(); }, [loadDashboard]);
  useEffect(() => { void loadProjects(); }, [loadProjects]);
  useEffect(() => { void appApi.recoverContinuations().catch(() => undefined); }, []);
  useEffect(() => { mainRef.current?.focus(); }, [location.pathname]);
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        navigate("/search");
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate]);
  useEffect(() => {
    let cancelled = false;
    let timer: number | undefined;
    void appApi.settings().then((settings) => {
      if (cancelled || !settings.autoWatch) return;
      const interval = Math.max(15, Math.min(60, settings.autoScanIntervalSeconds || 15)) * 1000;
      timer = window.setInterval(() => void pollChanges(), interval);
    }).catch(() => undefined);
    return () => { cancelled = true; if (timer) window.clearInterval(timer); };
  }, [pollChanges]);

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true"><span /><span /><span /></div>
          <div><strong>Continuum</strong><small>UNIFIED / LOCAL</small></div>
        </div>
        <div className="relay-track" aria-hidden="true"><i /><i /><i /></div>
        <nav aria-label="主导航">
          {navigation.map(({ to, label, icon: Icon }) => (
            <NavLink key={to} to={to} className={({ isActive }) => `nav-item ${isActive ? "active" : ""}`}>
              <Icon size={17} strokeWidth={1.8} /><span>{label}</span>
            </NavLink>
          ))}
        </nav>
        <button className="sidebar-create" onClick={() => navigate("/projects?create=1")}>
          <Link2 size={16} /> 创建统一项目
        </button>
        <div className="privacy-note"><span className="status-led ok" />本地优先<br /><small>无网络传输</small></div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div className="project-context"><span>本地客户端</span><strong>Continuum</strong><code>unified context</code></div>
          <button className="scan-control" onClick={() => void scanSessions()} disabled={scanning} data-testid="scan-sessions-btn">
            <RefreshCw size={15} className={scanning ? "animate-spin" : ""} />
            <span>{scanning ? "扫描中" : "扫描 Codex 会话"}</span>
          </button>
        </header>
        <main ref={mainRef} tabIndex={-1} className="main-content"><Outlet /></main>
        <footer className="statusbar">
          <div><span className={`status-led ${error ? "error" : "ok"}`} />数据库 {error ? "异常" : "已连接"}</div>
          <div><span className={`status-led ${watcherErrorCount ? "error" : scanning ? "pulse" : "ok"}`} />{scanning ? "正在扫描" : watcherErrorCount ? `监听错误 ${watcherErrorCount}` : dashboard?.lastScanAt ? "增量监听中" : "等待首次扫描"}</div>
          <div className="status-spacer" />
          <div>来源会话 {dashboard?.sessionCount ?? "—"}</div><div>统一项目 {projects.filter((project) => !project.archived).length}</div>
        </footer>
      </section>

      <section className="toast-region" aria-live="polite" aria-label="通知">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast toast-${toast.tone}`} role={toast.tone === "error" ? "alert" : "status"}>
            <ToastIcon tone={toast.tone} /><div><strong>{toast.title}</strong>{toast.detail && <p>{toast.detail}</p>}</div>
            <button aria-label="关闭通知" onClick={() => dismissToast(toast.id)}><X size={15} /></button>
          </div>
        ))}
      </section>
    </div>
  );
}
