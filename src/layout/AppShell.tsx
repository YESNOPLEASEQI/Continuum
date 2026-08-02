import { useGSAP } from "@gsap/react";
import gsap from "gsap";
import { Activity, Braces, Database, RefreshCw, X } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { appApi } from "../api/bridge";
import { AppServerApprovalDialog } from "../components/AppServerApprovalDialog";
import { ToastIcon } from "../components/ui";
import {
  ContinuumMotionProvider,
} from "../motion/ContinuumMotion";
import { useMajorSessionScan } from "../motion/useMajorSessionScan";
import { useAppStore } from "../store/appStore";
import { GlobalOverlayMenu } from "./GlobalOverlayMenu";

gsap.registerPlugin(useGSAP);

const routeLabels: Array<[RegExp, string]> = [
  [/^\/projects$/, "Project archive"],
  [/^\/projects\/[^/]+\/chat$/, "Project desk"],
  [/^\/projects\/[^/]+\/continuation$/, "Fresh continuation"],
  [/^\/projects\/[^/]+\/context$/, "Context inspector"],
  [/^\/sessions$/, "Source sessions"],
  [/^\/sessions\/[^/]+$/, "Session detail"],
  [/^\/configurations$/, "Skills / MCP"],
  [/^\/profiles$/, "Profiles"],
  [/^\/search$/, "Search"],
  [/^\/diagnostics$/, "Diagnostics"],
  [/^\/settings$/, "Settings"],
];

export default function AppShell() {
  return (
    <ContinuumMotionProvider>
      <AppShellContent />
    </ContinuumMotionProvider>
  );
}

function AppShellContent() {
  const location = useLocation();
  const navigate = useNavigate();
  const shellRef = useRef<HTMLDivElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const scanWithTransition = useMajorSessionScan();
  const {
    dashboard,
    projects,
    scanning,
    error,
    toasts,
    watcherErrorCount,
    loadDashboard,
    loadProjects,
    pollChanges,
    dismissToast,
  } = useAppStore();

  const routeLabel = useMemo(
    () => routeLabels.find(([pattern]) => pattern.test(location.pathname))?.[1] ?? "Local archive",
    [location.pathname],
  );
  const projectId = location.pathname.match(/^\/projects\/([^/]+)/)?.[1];
  const activeProject = projectId ? projects.find((project) => project.id === projectId) : null;
  const isSideSheet = /^\/sessions\/[^/]+$/.test(location.pathname) || /\/context$/.test(location.pathname);

  useEffect(() => { void loadDashboard(); }, [loadDashboard]);
  useEffect(() => { void loadProjects(); }, [loadProjects]);
  useEffect(() => { void appApi.recoverContinuations().catch(() => undefined); }, []);
  useEffect(() => {
    if (location.pathname !== "/search") mainRef.current?.focus();
  }, [location.pathname]);
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
    return () => {
      cancelled = true;
      if (timer) window.clearInterval(timer);
    };
  }, [pollChanges]);

  useGSAP(
    () => {
      const page = mainRef.current?.firstElementChild;
      if (!page) return;
      const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
      gsap.killTweensOf(page);
      gsap.fromTo(
        page,
        {
          autoAlpha: reduced ? 1 : 0,
          x: reduced || !isSideSheet ? 0 : 38,
          y: reduced || isSideSheet ? 0 : 18,
          clipPath: reduced
            ? "inset(0 0 0 0)"
            : isSideSheet
              ? "inset(0 0 0 6%)"
              : "inset(0 0 7% 0)",
        },
        {
          autoAlpha: 1,
          y: 0,
          clipPath: "inset(0 0 0 0)",
          duration: reduced ? 0 : 0.56,
          ease: "power3.out",
          overwrite: true,
          clearProps: "transform,opacity,visibility,clipPath",
        },
      );
    },
    { dependencies: [isSideSheet, location.pathname], scope: shellRef, revertOnUpdate: true },
  );

  return (
    <div ref={shellRef} className="archive-app-shell">
      <header className="archive-app-header">
        <button className="archive-brand" onClick={() => navigate("/projects")}>
          <span aria-hidden="true"><i /><i /><i /></span>
          <strong>CONTINUUM</strong>
          <small>LOCAL THREAD ARCHIVE</small>
        </button>
        <div className="archive-route-context">
          <span>{routeLabel}</span>
          <i />
          <strong>{activeProject?.name ?? "Continuum"}</strong>
          {activeProject && <code>{activeProject.currentBranchName}</code>}
        </div>
        <button
          className="archive-scan-control"
          onClick={() => void scanWithTransition()}
          disabled={scanning}
          data-testid="scan-sessions-btn"
        >
          <RefreshCw size={14} className={scanning ? "animate-spin" : ""} />
          <span>{scanning ? "Indexing" : "Re-index"}</span>
        </button>
      </header>

      <GlobalOverlayMenu />

      <main ref={mainRef} tabIndex={-1} className="app-main-scroll">
        <Outlet />
      </main>

      <footer className="archive-statusbar">
        <div><Database size={12} /><span className={`status-led ${error ? "error" : "ok"}`} />SQLite {error ? "error" : "connected"}</div>
        <div><Activity size={12} /><span className={`status-led ${watcherErrorCount ? "error" : scanning ? "pulse" : "ok"}`} />{scanning ? "Indexing real sessions" : watcherErrorCount ? `${watcherErrorCount} watcher errors` : "Watching local changes"}</div>
        <div className="archive-statusbar-spacer" />
        <div><Braces size={12} />{dashboard?.sessionCount ?? 0} sessions</div>
        <div>{projects.filter((project) => !project.archived).length} projects</div>
      </footer>

      <section className="toast-region" aria-live="polite" aria-label="通知">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast toast-${toast.tone}`} role={toast.tone === "error" ? "alert" : "status"}>
            <ToastIcon tone={toast.tone} />
            <div><strong>{toast.title}</strong>{toast.detail && <p>{toast.detail}</p>}</div>
            <button aria-label="关闭通知" onClick={() => dismissToast(toast.id)}><X size={15} /></button>
          </div>
        ))}
      </section>
      <AppServerApprovalDialog />
    </div>
  );
}
