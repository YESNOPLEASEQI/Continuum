import { useGSAP } from "@gsap/react";
import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import {
  ArrowDown,
  ArrowUpRight,
  Clock3,
  FolderKanban,
  FolderOpen,
  GitBranch,
  Plus,
  RadioTower,
  Sparkles,
} from "lucide-react";
import { useMemo, useRef } from "react";
import type {
  ContextHealthLevel,
  SessionSummary,
  UnifiedProjectSummary,
} from "../types/models";
import { EmptyState, ErrorState, LoadingState, PathText } from "./ui";

gsap.registerPlugin(useGSAP, ScrollTrigger);

const healthLabels: Record<ContextHealthLevel, string> = {
  healthy: "Stable",
  growing: "Growing",
  compression_recommended: "Compress",
  fresh_continuation_recommended: "Fresh advised",
  critical: "Critical",
};

interface ProjectArchiveDeckProps {
  projects: UnifiedProjectSummary[];
  sessions: SessionSummary[];
  loading: boolean;
  error: string | null;
  onRetry: () => void;
  onOpenProject: (project: UnifiedProjectSummary) => void;
  onOpenSession: (session: SessionSummary) => void;
  onBrowseSessions: () => void;
  onCreateProject: () => void;
  onImportProject: () => void;
}

function prefersReducedMotion() {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
}

export function ProjectArchiveDeck({
  projects,
  sessions,
  loading,
  error,
  onRetry,
  onOpenProject,
  onOpenSession,
  onBrowseSessions,
  onCreateProject,
  onImportProject,
}: ProjectArchiveDeckProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const activeProjects = useMemo(
    () => projects.filter((project) => !project.archived),
    [projects],
  );
  const recentSessions = useMemo(
    () => [...sessions].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)).slice(0, 8),
    [sessions],
  );

  useGSAP(
    () => {
      const root = rootRef.current;
      if (!root) return;
      const reduced = prefersReducedMotion();
      const letters = root.querySelectorAll(".archive-wordmark-letter");
      const introLines = root.querySelectorAll(".archive-intro-reveal");
      const cards = gsap.utils.toArray<HTMLElement>(".archive-project-sheet", root);
      const scroller = root.closest<HTMLElement>(".app-main-scroll") ?? undefined;

      if (!reduced) {
        gsap.fromTo(
          letters,
          { yPercent: 112 },
          {
            yPercent: 0,
            duration: 0.9,
            ease: "power4.out",
            stagger: 0.04,
            clearProps: "transform",
          },
        );
        gsap.fromTo(
          introLines,
          { autoAlpha: 0, y: 14 },
          {
            autoAlpha: 1,
            y: 0,
            duration: 0.58,
            ease: "power3.out",
            stagger: 0.07,
            delay: 0.42,
            clearProps: "transform,opacity,visibility",
          },
        );
      }

      if (!cards.length) return;
      if (reduced || cards.length === 1) {
        gsap.set(cards, { clearProps: "all" });
        return;
      }

      cards.forEach((card, index) => {
        gsap.set(card, {
          yPercent: -50 + index * 5.25,
          scale: 1 - index * 0.065,
          zIndex: cards.length - index,
          transformOrigin: "50% 0%",
          autoAlpha: index > 4 ? 0 : 1,
        });
      });

      const timeline = gsap.timeline({
        scrollTrigger: {
          id: "continuum-project-archive",
          trigger: root.querySelector(".archive-deck-section"),
          scroller,
          start: "top top",
          end: () => `+=${Math.max(1, cards.length - 1) * Math.max(620, window.innerHeight * 0.78)}`,
          pin: root.querySelector(".archive-deck-pin"),
          pinSpacing: true,
          scrub: 0.8,
          invalidateOnRefresh: true,
        },
      });

      cards.slice(0, -1).forEach((card, index) => {
        const label = `sheet-${index}`;
        timeline.addLabel(label);
        timeline.to(
          card,
          {
            yPercent: -176,
            rotationX: 35,
            autoAlpha: 0,
            duration: 1,
            ease: "none",
          },
          label,
        );
        cards.slice(index + 1).forEach((nextCard, offset) => {
          timeline.to(
            nextCard,
            {
              yPercent: -50 + offset * 5.25,
              scale: 1 - offset * 0.065,
              autoAlpha: offset > 4 ? 0 : 1,
              duration: 1,
              ease: "none",
            },
            label,
          );
        });
      });

      requestAnimationFrame(() => ScrollTrigger.refresh());
    },
    {
      dependencies: [activeProjects.length],
      scope: rootRef,
      revertOnUpdate: true,
    },
  );

  return (
    <div ref={rootRef} className="archive-home">
      <section className="archive-home-intro" aria-labelledby="archive-home-title">
        <div className="archive-intro-kicker archive-intro-reveal">
          <span>LOCAL CODEX CONTINUITY</span>
          <span>{activeProjects.length} PROJECTS / {sessions.length} SESSIONS</span>
        </div>
        <h1 id="archive-home-title" className="archive-wordmark" aria-label="Continuum">
          {Array.from("CONTINUUM").map((letter, index) => (
            <span key={`${letter}-${index}`}><i className="archive-wordmark-letter" aria-hidden="true">{letter}</i></span>
          ))}
        </h1>
        <div className="archive-intro-bottom archive-intro-reveal">
          <p>把本地会话整理成可以继续工作的项目档案。</p>
          <div>
            <button onClick={onImportProject}><FolderOpen size={15} />导入目录</button>
            <button className="is-primary" onClick={onCreateProject}><Plus size={15} />创建项目</button>
          </div>
        </div>
        <div className="archive-scroll-cue archive-intro-reveal" aria-hidden="true">
          <span>Browse archive</span><ArrowDown size={14} />
        </div>
      </section>

      {loading && !projects.length ? (
        <section className="archive-home-state"><LoadingState label="正在读取项目档案" /></section>
      ) : error ? (
        <section className="archive-home-state"><ErrorState message={error} onRetry={onRetry} /></section>
      ) : !activeProjects.length ? (
        <section className="archive-home-state">
          <EmptyState
            icon={<FolderKanban size={24} />}
            title="档案桌还是空的"
            detail="选择一个真实工作目录，Continuum 会把已有 Codex 会话组织到同一项目中。"
            action={<button className="button button-primary" onClick={onCreateProject}>创建第一个项目</button>}
          />
        </section>
      ) : (
        <section className="archive-deck-section" aria-labelledby="archive-projects-title">
          <div className="archive-deck-pin">
            <header className="archive-deck-heading">
              <div><span>PROJECT ARCHIVE</span><h2 id="archive-projects-title">继续一项工作</h2></div>
              <p>滚动会移动档案层级，不会改变真实项目状态。</p>
            </header>
            <div className="archive-deck-scene">
              {activeProjects.map((project, index) => {
                const projectSessions = recentSessions.filter((session) => session.boundProjectId === project.id).slice(0, 2);
                return (
                  <article
                    className={`archive-project-sheet health-${project.health.level}`}
                    key={project.id}
                    data-project-index={index}
                  >
                    <div className="archive-sheet-spine">
                      <span>{String(index + 1).padStart(2, "0")}</span>
                      <strong>{new Date(project.updatedAt).getFullYear()}</strong>
                      <i />
                      <small>{project.pathExists ? "LIVE" : "MISSING"}</small>
                    </div>
                    <button
                      className="archive-sheet-body"
                      onClick={() => onOpenProject(project)}
                      disabled={!project.pathExists}
                    >
                      <header>
                        <div>
                          <span>UNIFIED PROJECT / {project.currentBranchName}</span>
                          <h3>{project.name}</h3>
                        </div>
                        <ArrowUpRight size={24} />
                      </header>
                      <div className="archive-sheet-thesis">
                        <p>{project.currentTask || project.goal}</p>
                        <PathText value={project.projectPath} />
                      </div>
                      <div className="archive-sheet-readouts">
                        <div><GitBranch size={13} /><span>Branch</span><strong>{project.currentBranchName}</strong></div>
                        <div><RadioTower size={13} /><span>Threads</span><strong>{project.sessionCount}</strong></div>
                        <div><Clock3 size={13} /><span>Updated</span><strong>{new Date(project.updatedAt).toLocaleDateString("zh-CN", { month: "2-digit", day: "2-digit" })}</strong></div>
                      </div>
                      <div className="archive-health-track">
                        <span><i style={{ width: `${Math.min(100, Math.round(project.health.thresholdRatio * 100))}%` }} /></span>
                        <strong>{healthLabels[project.health.level]}</strong>
                        <small>{Math.round(project.health.thresholdRatio * 100)}% context</small>
                      </div>
                      <footer>
                        <span>RECENT SESSIONS</span>
                        <div>
                          {projectSessions.length ? projectSessions.map((session) => (
                            <em key={session.id}>{session.title}</em>
                          )) : <em>No bound session in recent index</em>}
                        </div>
                        <strong>Open archive</strong>
                      </footer>
                    </button>
                  </article>
                );
              })}
            </div>
            <div className="archive-deck-counter" aria-hidden="true">
              <span>01</span><i /><span>{String(activeProjects.length).padStart(2, "0")}</span>
            </div>
          </div>
        </section>
      )}

      <section className="archive-recent-sessions" aria-labelledby="recent-session-title">
        <header>
          <div><span>RECENT SESSIONS</span><h2 id="recent-session-title">最近发生的工作</h2></div>
          <button onClick={onBrowseSessions}>浏览来源会话 <ArrowUpRight size={15} /></button>
        </header>
        <div className="archive-session-ledger">
          {recentSessions.map((session, index) => (
            <button key={session.id} onClick={() => onOpenSession(session)}>
              <code>{String(index + 1).padStart(2, "0")}</code>
              <span><strong>{session.title}</strong><small>{session.boundProjectName ?? session.workingDirectory ?? "未绑定项目"}</small></span>
              <em>{session.clientKind === "desktop" ? "DESKTOP" : session.clientKind === "cli" ? "CLI" : "CODEX"}</em>
              <time>{new Date(session.updatedAt).toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" })}</time>
              <ArrowUpRight size={15} />
            </button>
          ))}
          {!recentSessions.length && (
            <div className="archive-session-empty"><Sparkles size={16} /><span>完成一次真实会话扫描后，最近工作会出现在这里。</span></div>
          )}
        </div>
      </section>
    </div>
  );
}
