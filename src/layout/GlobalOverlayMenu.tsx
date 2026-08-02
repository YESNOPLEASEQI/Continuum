import { useGSAP } from "@gsap/react";
import gsap from "gsap";
import { CustomEase } from "gsap/CustomEase";
import {
  Activity,
  ArrowUpRight,
  Braces,
  FolderKanban,
  GitBranch,
  RadioTower,
  Search,
  Settings,
  SlidersHorizontal,
  Sparkles,
  Stethoscope,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useContinuumMotion } from "../motion/ContinuumMotion";
import { useAppStore } from "../store/appStore";

gsap.registerPlugin(useGSAP, CustomEase);

const menuEase = CustomEase.create(
  "continuumArchiveMenu",
  "M0,0 C0.85,0 0.15,1 1,1",
);

const primaryNavigation = [
  { to: "/projects", label: "Projects", detail: "项目档案", icon: FolderKanban },
  { to: "/sessions", label: "Sessions", detail: "来源会话", icon: RadioTower },
  { to: "/search", label: "Search", detail: "搜索与命令", icon: Search },
] as const;

const systemNavigation = [
  { to: "/configurations", label: "Skills / MCP", icon: Sparkles },
  { to: "/profiles", label: "Profiles", icon: SlidersHorizontal },
  { to: "/diagnostics", label: "Diagnostics", icon: Stethoscope },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

function prefersReducedMotion() {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
}

export function GlobalOverlayMenu() {
  const navigate = useNavigate();
  const location = useLocation();
  const { navigateMajor } = useContinuumMotion();
  const projects = useAppStore((state) => state.projects);
  const dashboard = useAppStore((state) => state.dashboard);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const timelineRef = useRef<gsap.core.Timeline | null>(null);
  const desiredOpenRef = useRef(false);
  const pendingNavigationRef = useRef<{ path: string; major: boolean } | null>(null);
  const [isOpen, setIsOpen] = useState(false);

  const { contextSafe } = useGSAP(
    () => {
      const reduced = prefersReducedMotion();
      const root = rootRef.current;
      if (!root) return;
      const leftBlade = root.querySelector(".overlay-blade-left");
      const rightBlade = root.querySelector(".overlay-blade-right");
      const content = root.querySelector(".overlay-menu-content");
      const lines = root.querySelectorAll(".overlay-menu-line-inner");
      const rules = root.querySelectorAll(".overlay-menu-rule");
      const topBar = triggerRef.current?.querySelector(".menu-bar-top");
      const bottomBar = triggerRef.current?.querySelector(".menu-bar-bottom");

      gsap.set(root, { autoAlpha: 0, pointerEvents: "none" });
      gsap.set(leftBlade, {
        rotation: reduced ? 0 : 180,
        scale: reduced ? 1 : 2,
        transformOrigin: "100% 50%",
      });
      gsap.set(rightBlade, {
        rotation: reduced ? 0 : -180,
        scale: reduced ? 1 : 2,
        transformOrigin: "0% 50%",
      });
      gsap.set(content, { autoAlpha: 0 });
      gsap.set(lines, { yPercent: reduced ? 0 : 112 });
      gsap.set(rules, { scaleX: reduced ? 1 : 0, transformOrigin: "0% 50%" });

      const timeline = gsap.timeline({
        paused: true,
        defaults: { overwrite: "auto" },
        onComplete: () => {
          root.querySelector<HTMLElement>(".overlay-menu-primary button")?.focus();
        },
        onReverseComplete: () => {
          gsap.set(root, { autoAlpha: 0, pointerEvents: "none" });
          setIsOpen(false);
          const pending = pendingNavigationRef.current;
          pendingNavigationRef.current = null;
          if (pending) {
            if (pending.major) void navigateMajor(pending.path, "打开项目档案");
            else navigate(pending.path);
          } else {
            triggerRef.current?.focus();
          }
        },
      });

      timeline
        .set(root, { autoAlpha: 1, pointerEvents: "auto" })
        .to(
          [leftBlade, rightBlade],
          {
            rotation: 0,
            scale: reduced ? 1 : 2,
            duration: reduced ? 0 : 1.04,
            ease: menuEase,
          },
          0,
        )
        .to(
          [topBar, bottomBar],
          {
            y: 0,
            rotation: (index) => (index === 0 ? 45 : -45),
            duration: reduced ? 0 : 0.48,
            ease: "power3.inOut",
          },
          0.08,
        )
        .to(content, { autoAlpha: 1, duration: reduced ? 0 : 0.12 }, reduced ? 0 : 0.5)
        .to(
          lines,
          {
            yPercent: 0,
            duration: reduced ? 0 : 0.62,
            ease: "power3.out",
            stagger: reduced ? 0 : 0.065,
          },
          reduced ? 0 : 0.56,
        )
        .to(
          rules,
          {
            scaleX: 1,
            duration: reduced ? 0 : 0.52,
            ease: "power3.out",
            stagger: reduced ? 0 : 0.05,
          },
          reduced ? 0 : 0.68,
        );
      timelineRef.current = timeline;

      return () => {
        timeline.kill();
        timelineRef.current = null;
      };
    },
    { scope: rootRef },
  );

  const closeMenu = contextSafe(() => {
    desiredOpenRef.current = false;
    timelineRef.current?.reverse();
  });

  const toggleMenu = contextSafe(() => {
    const timeline = timelineRef.current;
    if (!timeline) return;
    desiredOpenRef.current = !desiredOpenRef.current;
    if (desiredOpenRef.current) {
      setIsOpen(true);
      timeline.play();
    } else {
      timeline.reverse();
    }
  });

  function chooseDestination(path: string, major = false) {
    if (path === location.pathname) {
      closeMenu();
      return;
    }
    pendingNavigationRef.current = { path, major };
    closeMenu();
  }

  useEffect(() => {
    if (!isOpen) return;
    const previousOverflow = document.documentElement.style.overflow;
    document.documentElement.style.overflow = "hidden";
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closeMenu();
        return;
      }
      if (event.key !== "Tab" || !rootRef.current) return;
      const focusable = Array.from(
        rootRef.current.querySelectorAll<HTMLElement>(
          'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      );
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable.at(-1);
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.documentElement.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [closeMenu, isOpen]);

  const activeProjects = projects.filter((project) => !project.archived).slice(0, 4);

  return (
    <>
      <button
        ref={triggerRef}
        className={`global-menu-trigger ${isOpen ? "is-open" : ""}`}
        type="button"
        aria-expanded={isOpen}
        aria-controls="continuum-global-menu"
        aria-label={isOpen ? "关闭全局菜单" : "打开全局菜单"}
        onClick={toggleMenu}
      >
        <span className="global-menu-trigger-label">{isOpen ? "Close" : "Menu"}</span>
        <span className="global-menu-trigger-bars" aria-hidden="true">
          <i className="menu-bar-top" />
          <i className="menu-bar-bottom" />
        </span>
      </button>

      <div
        ref={rootRef}
        id="continuum-global-menu"
        className="global-overlay-menu"
        role="dialog"
        aria-modal="true"
        aria-label="Continuum 全局菜单"
        aria-hidden={!isOpen}
      >
        <div className="overlay-blade overlay-blade-left" aria-hidden="true" />
        <div className="overlay-blade overlay-blade-right" aria-hidden="true" />
        <div className="overlay-menu-content">
          <header className="overlay-menu-header">
            <button className="overlay-menu-brand" onClick={() => chooseDestination("/projects")}>
              <span>CONTINUUM</span>
              <small>LOCAL THREAD ARCHIVE</small>
            </button>
            <div className="overlay-menu-runtime">
              <span><Activity size={13} />LOCAL</span>
              <strong>{dashboard?.sessionCount ?? 0} sessions</strong>
              <strong>{projects.filter((project) => !project.archived).length} projects</strong>
            </div>
          </header>

          <div className="overlay-menu-grid">
            <nav className="overlay-menu-primary" aria-label="主要导航">
              {primaryNavigation.map(({ to, label, detail, icon: Icon }) => (
                <button
                  key={to}
                  className={location.pathname.startsWith(to) ? "is-current" : ""}
                  onClick={() => chooseDestination(to)}
                >
                  <span className="overlay-menu-line"><span className="overlay-menu-line-inner"><Icon size={17} /><small>{detail}</small><strong>{label}</strong><ArrowUpRight size={20} /></span></span>
                  <i className="overlay-menu-rule" />
                </button>
              ))}
            </nav>

            <section className="overlay-menu-projects" aria-labelledby="overlay-projects-title">
              <div className="overlay-menu-line">
                <div className="overlay-menu-line-inner overlay-menu-section-heading">
                  <span id="overlay-projects-title">Recent projects</span>
                  <small>最近打开</small>
                </div>
              </div>
              <i className="overlay-menu-rule" />
              {activeProjects.map((project, index) => (
                <button
                  key={project.id}
                  onClick={() => chooseDestination(`/projects/${project.id}/chat`, true)}
                  disabled={!project.pathExists}
                >
                  <span className="overlay-menu-line"><span className="overlay-menu-line-inner"><code>{String(index + 1).padStart(2, "0")}</code><span><strong>{project.name}</strong><small>{project.currentTask || project.goal}</small></span><GitBranch size={15} /></span></span>
                  <i className="overlay-menu-rule" />
                </button>
              ))}
              {!activeProjects.length && (
                <div className="overlay-menu-line"><p className="overlay-menu-line-inner overlay-menu-empty">尚未创建统一项目</p></div>
              )}
            </section>
          </div>

          <footer className="overlay-menu-footer">
            <nav aria-label="系统导航">
              {systemNavigation.map(({ to, label, icon: Icon }) => (
                <button key={to} onClick={() => chooseDestination(to)}>
                  <span className="overlay-menu-line"><span className="overlay-menu-line-inner"><Icon size={13} />{label}</span></span>
                </button>
              ))}
            </nav>
            <div className="overlay-menu-line">
              <p className="overlay-menu-line-inner"><Braces size={13} />App Server / CLI fallback / SQLite v4</p>
            </div>
          </footer>
        </div>
      </div>
    </>
  );
}
