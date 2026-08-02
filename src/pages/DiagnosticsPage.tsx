import { save } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle,
  CheckCircle2,
  Clipboard,
  Database,
  Download,
  FolderOpen,
  HardDrive,
  RefreshCw,
  ShieldCheck,
  Stethoscope,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";
import { appApi } from "../api/bridge";
import {
  Badge,
  ConfirmDialog,
  ErrorState,
  LoadingState,
  PageHeader,
  PathText,
} from "../components/ui";
import { useAppStore } from "../store/appStore";
import { useMajorSessionScan } from "../motion/useMajorSessionScan";
import type { DatabaseBackupRecord, DiagnosticsReport } from "../types/models";

export default function DiagnosticsPage() {
  const notify = useAppStore((state) => state.notify);
  const scanSessions = useMajorSessionScan();
  const [report, setReport] = useState<DiagnosticsReport | null>(null);
  const [backups, setBackups] = useState<DatabaseBackupRecord[]>([]);
  const [restore, setRestore] = useState<DatabaseBackupRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [appServerProbe, setAppServerProbe] = useState<string | null>(null);

  async function load(forceCodex = false) {
    setLoading(true);
    try {
      const [next, items] = await Promise.all([
        appApi.diagnostics(forceCodex),
        appApi.databaseBackups(),
      ]);
      setReport(next);
      setBackups(items);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    void load();
  }, []);

  async function copyReport() {
    const value = await appApi.diagnosticsReport();
    await navigator.clipboard.writeText(value);
    notify({ tone: "success", title: "脱敏诊断报告已复制" });
  }
  async function exportReport() {
    const path = await save({
      defaultPath: "continuum-diagnostics.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    const output = await appApi.exportDiagnostics(path);
    notify({ tone: "success", title: "脱敏诊断报告已导出", detail: output });
  }
  async function createBackup() {
    const created = await appApi.createDatabaseBackup("diagnostics_manual");
    await load();
    notify({
      tone: "success",
      title: "数据库备份已创建",
      detail: created.path,
    });
  }
  async function restoreBackup() {
    if (!restore) return;
    const health = await appApi.restoreDatabaseBackup(restore.path);
    setRestore(null);
    await load();
    notify({
      tone: "success",
      title: "数据库已恢复并验证",
      detail: `${health.integrity} · schema ${health.schemaVersion}`,
    });
  }
  async function probeAppServer() {
    try {
      const value = await appApi.probeCodexAppServer();
      setAppServerProbe(value);
      notify({
        tone: "success",
        title: "Codex App Server 握手成功",
        detail: value,
      });
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setAppServerProbe(`失败：${message}`);
      notify({
        tone: "error",
        title: "Codex App Server 握手失败",
        detail: message,
      });
    }
  }

  if (loading && !report) return <LoadingState label="正在收集本机诊断信息" />;
  if (error || !report)
    return (
      <div className="page">
        <ErrorState
          message={error ?? "无法读取诊断信息"}
          onRetry={() => void load()}
        />
      </div>
    );
  return (
    <div className="page diagnostics-page">
      <PageHeader
        eyebrow="LOCAL DIAGNOSTICS"
        title="Diagnostics"
        description="检查 Continuum、Codex、会话目录、监听器和 SQLite。复制与导出的报告会自动脱敏用户名、家目录和秘密值。"
        actions={
          <>
            <button
              className="button button-secondary"
              onClick={() => void copyReport()}
            >
              <Clipboard size={14} />
              复制脱敏报告
            </button>
            <button
              className="button button-secondary"
              onClick={() => void exportReport()}
            >
              <Download size={14} />
              导出报告
            </button>
            <button
              className="button button-primary"
              onClick={() => void load(true)}
              disabled={loading}
            >
              <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
              重新检测
            </button>
          </>
        }
      />
      <div className="diagnostic-summary">
        <DiagnosticCard
          icon={<Stethoscope />}
          label="Continuum"
          value={`v${report.continuumVersion}`}
          detail={report.osVersion}
          ok
        />
        <DiagnosticCard
          icon={<ShieldCheck />}
          label="Codex"
          value={report.codex.version ?? "未安装"}
          detail={`${report.codex.supportsAppServer ? "App Server 可用" : "App Server 不可用"} · ${report.codex.executablePath ?? report.codex.error ?? "无路径"}`}
          ok={report.codex.installed}
        />
        <DiagnosticCard
          icon={<Database />}
          label="SQLite"
          value={`schema ${report.database.schemaVersion}`}
          detail={`${report.database.integrity} · ${(report.database.sizeBytes / 1024 / 1024).toFixed(2)} MB`}
          ok={
            report.database.integrity === "ok" &&
            !report.database.orphanNodes &&
            !report.database.invalidBindings
          }
        />
        <DiagnosticCard
          icon={<HardDrive />}
          label="监听器"
          value={report.watcherEnabled ? "已启用" : "已关闭"}
          detail={`${report.watcherIntervalSeconds}s · ${report.recentScan ?? "尚无扫描"}`}
          ok={report.watcherEnabled}
        />
      </div>
      <div className="diagnostic-grid">
        <section>
          <header>
            <div>
              <p className="eyebrow">RUNTIME</p>
              <h2>运行环境</h2>
            </div>
          </header>
          <dl className="diagnostic-list">
            <div>
              <dt>Windows</dt>
              <dd>{report.osVersion}</dd>
            </div>
            <div>
              <dt>WebView2</dt>
              <dd>{report.webviewVersion ?? "系统未公开版本"}</dd>
            </div>
            <div>
              <dt>Node.js</dt>
              <dd>{report.nodeVersion ?? "未检测到"}</dd>
            </div>
            <div>
              <dt>Rust</dt>
              <dd>{report.rustVersion ?? "未检测到"}</dd>
            </div>
            <div>
              <dt>最近 Continuation</dt>
              <dd>{report.recentContinuation ?? "尚无记录"}</dd>
            </div>
            <div>
              <dt>App Server 探针</dt>
              <dd>{appServerProbe ?? "尚未执行"}</dd>
            </div>
          </dl>
          <div className="diagnostic-actions">
            <button
              className="button button-secondary"
              onClick={() => void openPath(report.logDirectory)}
            >
              <FolderOpen size={13} />
              打开日志目录
            </button>
            <button
              className="button button-secondary"
              onClick={() => void openPath(report.dataDirectory)}
            >
              <FolderOpen size={13} />
              打开数据库目录
            </button>
            <button
              className="button button-secondary"
              onClick={() => void probeAppServer()}
              disabled={!report.codex.supportsAppServer}
            >
              <Stethoscope size={13} />
              测试 App Server
            </button>
          </div>
        </section>
        <section>
          <header>
            <div>
              <p className="eyebrow">SESSION PATHS</p>
              <h2>会话目录</h2>
            </div>
            <button className="text-button" onClick={() => void scanSessions()}>
              <RefreshCw size={13} />
              重新扫描
            </button>
          </header>
          <div className="path-health-list">
            {report.sessionPaths.map((item) => (
              <div key={item.path}>
                <span
                  className={`status-led ${item.readable ? "ok" : "error"}`}
                />
                <PathText value={item.path} />
                <Badge tone={item.readable ? "success" : "danger"}>
                  {item.exists ? (item.readable ? "可读" : "不可读") : "不存在"}
                </Badge>
              </div>
            ))}
          </div>
        </section>
        <section>
          <header>
            <div>
              <p className="eyebrow">RECENT ERRORS</p>
              <h2>最近错误</h2>
            </div>
            <Badge tone={report.recentErrors.length ? "warning" : "success"}>
              {report.recentErrors.length}
            </Badge>
          </header>
          {report.recentErrors.length ? (
            <ul className="diagnostic-errors">
              {report.recentErrors.map((item, index) => (
                <li key={`${index}-${item}`}>
                  <AlertTriangle size={12} />
                  {item}
                </li>
              ))}
            </ul>
          ) : (
            <div className="diagnostic-ok">
              <CheckCircle2 size={18} />
              没有未解决的扫描或诊断错误
            </div>
          )}
        </section>
        <section>
          <header>
            <div>
              <p className="eyebrow">DATABASE RECOVERY</p>
              <h2>备份与恢复</h2>
            </div>
            <button
              className="button button-secondary"
              onClick={() => void createBackup()}
            >
              <Database size={13} />
              立即备份
            </button>
          </header>
          <div className="backup-list">
            {backups.length ? (
              backups.map((item) => (
                <button key={item.id} onClick={() => setRestore(item)}>
                  <span>
                    <strong>
                      {new Date(item.createdAt).toLocaleString("zh-CN")}
                    </strong>
                    <small>
                      {item.reason} · schema {item.schemaVersion} ·{" "}
                      {(item.sizeBytes / 1024).toFixed(1)} KB
                    </small>
                  </span>
                  <code>{item.sha256.slice(0, 12)}</code>
                </button>
              ))
            ) : (
              <p>尚无数据库备份。Migration 前备份会自动记录在这里。</p>
            )}
          </div>
        </section>
      </div>
      <ConfirmDialog
        open={Boolean(restore)}
        title="恢复这份数据库备份？"
        description="Continuum 会先自动备份当前数据库，再替换并执行完整性与 schema 验证。不会复制或删除源码和 Codex sessions。"
        confirmLabel="备份当前库并恢复"
        destructive
        onConfirm={() => void restoreBackup()}
        onCancel={() => setRestore(null)}
      />
    </div>
  );
}

function DiagnosticCard({
  icon,
  label,
  value,
  detail,
  ok,
}: {
  icon: ReactElement;
  label: string;
  value: string;
  detail: string;
  ok: boolean;
}) {
  return (
    <article className={ok ? "ok" : "warning"}>
      <div>{icon}</div>
      <span>{label}</span>
      <strong>{value}</strong>
      <small title={detail}>{detail}</small>
    </article>
  );
}
