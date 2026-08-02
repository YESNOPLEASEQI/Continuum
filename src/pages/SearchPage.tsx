import { ArrowRight, Command, FileSearch, Search, TerminalSquare } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { appApi } from "../api/bridge";
import { Badge, EmptyState, PageHeader } from "../components/ui";
import { useAppStore } from "../store/appStore";
import type { GlobalSearchResult } from "../types/models";

const commands = [
  { id: "projects", label: "打开统一项目", path: "/projects" },
  { id: "create", label: "创建统一项目", path: "/projects?create=1" },
  { id: "sessions", label: "绑定或扫描 Codex 会话", path: "/sessions" },
  { id: "profiles", label: "打开 Codex Profiles", path: "/profiles" },
  { id: "configurations", label: "打开 Skills 与 MCP", path: "/configurations" },
  { id: "diagnostics", label: "打开 Diagnostics", path: "/diagnostics" },
  { id: "settings", label: "打开设置", path: "/settings" },
];

export default function SearchPage() {
  const navigate = useNavigate();
  const scanSessions = useAppStore((state) => state.scanSessions);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<GlobalSearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const visibleCommands = useMemo(() => commands.filter((item) => !query.trim() || item.label.toLowerCase().includes(query.toLowerCase())), [query]);

  useEffect(() => { inputRef.current?.focus(); }, []);
  useEffect(() => {
    const value = query.trim();
    if (!value) { setResults([]); setLoading(false); return; }
    const timer = window.setTimeout(() => {
      setLoading(true);
      void appApi.globalSearch(value).then(setResults).finally(() => setLoading(false));
    }, 180);
    return () => window.clearTimeout(timer);
  }, [query]);

  function openResult(result: GlobalSearchResult) {
    if (result.projectId && result.branchId) navigate(`/projects/${result.projectId}/chat?branch=${result.branchId}&node=${result.id}`);
    else if (result.projectId) navigate(`/projects/${result.projectId}/chat`);
    else if (result.sessionId) navigate(`/sessions/${result.sessionId}`);
    else if (result.kind === "skill" || result.kind === "mcp") navigate("/configurations");
  }

  const rows = [...visibleCommands.map((item) => ({ type: "command" as const, item })), ...results.map((item) => ({ type: "result" as const, item }))];
  return (
    <div className="page search-page">
      <PageHeader eyebrow="GLOBAL SEARCH & COMMANDS" title="搜索与命令面板" description="搜索项目、分支、会话、消息、命令、错误、文件、CONTINUATION_ID、Skills 与 MCP。按 Ctrl+K 可随时打开。" />
      <div className="command-search">
        <Search size={18} />
        <input ref={inputRef} value={query} onChange={(event) => { setQuery(event.target.value); setActive(0); }} placeholder="输入搜索内容或命令…" onKeyDown={(event) => {
          if (event.key === "ArrowDown") { event.preventDefault(); setActive((value) => Math.min(rows.length - 1, value + 1)); }
          if (event.key === "ArrowUp") { event.preventDefault(); setActive((value) => Math.max(0, value - 1)); }
          if (event.key === "Enter" && rows[active]) { const row = rows[active]; if (row.type === "command") navigate(row.item.path); else openResult(row.item); }
        }} />
        <kbd>CTRL K</kbd>
      </div>
      <section className="search-results" aria-live="polite">
        <header><span>{loading ? "正在搜索本地索引" : `${results.length} 条内容结果`}</span><button className="text-button" onClick={() => void scanSessions()}><TerminalSquare size={13} />重新扫描会话</button></header>
        {!rows.length ? <EmptyState icon={<FileSearch size={22} />} title="输入关键词开始搜索" detail="搜索仅使用本地 SQLite 索引，不会上传项目或会话内容。" /> : rows.map((row, index) => row.type === "command" ? (
          <button key={`command-${row.item.id}`} className={`search-row command-row ${active === index ? "active" : ""}`} onMouseEnter={() => setActive(index)} onClick={() => navigate(row.item.path)}><Command size={15} /><span><strong>{row.item.label}</strong><small>命令</small></span><ArrowRight size={14} /></button>
        ) : (
          <button key={`${row.item.kind}-${row.item.id}`} className={`search-row ${active === index ? "active" : ""}`} onMouseEnter={() => setActive(index)} onClick={() => openResult(row.item)}><Badge tone={row.item.kind === "error" ? "danger" : row.item.kind === "project" ? "signal" : "neutral"}>{row.item.kind}</Badge><span><strong>{row.item.title}</strong><small>{row.item.excerpt}</small></span><code>{row.item.sessionId ?? row.item.branchId ?? row.item.id.slice(0, 12)}</code><ArrowRight size={14} /></button>
        ))}
      </section>
    </div>
  );
}
