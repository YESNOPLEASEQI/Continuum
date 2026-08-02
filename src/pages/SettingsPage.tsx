import { open } from "@tauri-apps/plugin-dialog";
import {
  Bot,
  Database,
  FolderCog,
  FolderPlus,
  Save,
  Settings2,
  ShieldCheck,
  Terminal,
  Trash2,
  Waypoints,
} from "lucide-react";
import { useEffect, useState } from "react";
import { appApi } from "../api/bridge";
import {
  ErrorState,
  LoadingState,
  PageHeader,
  PathText,
  Toggle,
} from "../components/ui";
import { useAppStore } from "../store/appStore";
import type { AppSettings, DiagnosticPathStatus } from "../types/models";

export default function SettingsPage() {
  const notify = useAppStore((state) => state.notify);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [pathStatuses, setPathStatuses] = useState<DiagnosticPathStatus[]>([]);
  useEffect(() => {
    appApi
      .settings()
      .then(setSettings)
      .catch((reason: unknown) =>
        setError(reason instanceof Error ? reason.message : String(reason)),
      );
  }, []);
  async function choosePath(title: string) {
    const path = await open({ directory: true, multiple: false, title });
    return path && !Array.isArray(path) ? path : null;
  }
  async function addSessionPath() {
    const path = await choosePath("选择 Codex 会话目录");
    if (path && settings && !settings.sessionPaths.includes(path))
      setSettings({
        ...settings,
        sessionPaths: [...settings.sessionPaths, path],
      });
  }
  async function save() {
    if (!settings) return;
    setSaving(true);
    try {
      const statuses = await appApi.validateSettingsPaths(settings);
      setPathStatuses(statuses);
      const unreadableSession = statuses.find(
        (item) => settings.sessionPaths.includes(item.path) && !item.readable,
      );
      if (unreadableSession) throw new Error(`会话目录不可读：${unreadableSession.path}`);
      setSettings(await appApi.saveSettings(settings));
      notify({ tone: "success", title: "Continuum 设置已保存" });
    } catch (reason) {
      notify({
        tone: "error",
        title: "保存失败",
        detail: reason instanceof Error ? reason.message : String(reason),
      });
    } finally {
      setSaving(false);
    }
  }
  if (error)
    return (
      <div className="page">
        <ErrorState message={error} />
      </div>
    );
  if (!settings) return <LoadingState label="正在读取 Continuum 本地设置" />;
  return (
    <div className="page settings-page">
      <PageHeader
        eyebrow="CONTINUUM SETTINGS"
        title="设置"
        description="控制 Agent 安装、会话监听、上下文编译和本地安全边界。"
        actions={
          <button
            className="button button-primary"
            onClick={() => void save()}
            disabled={saving}
            data-testid="save-settings-btn"
          >
            <Save size={14} />
            {saving ? "保存中" : "保存设置"}
          </button>
        }
      />
      {pathStatuses.length > 0 && (
        <div className="settings-path-health">
          {pathStatuses.map((item) => (
            <div key={item.path}><span className={`status-led ${item.readable || item.writable ? "ok" : "error"}`} /><PathText value={item.path} /><span>{item.exists ? item.readable ? "可读" : item.writable ? "可写" : "不可访问" : item.writable ? "父目录可写" : "不存在"}</span></div>
          ))}
        </div>
      )}
      <div className="settings-layout">
        <section className="settings-section">
          <header>
            <FolderCog size={18} />
            <div>
              <h2>工作区与会话</h2>
              <p>真实来源目录和 Fresh Continuation 的默认工作目录。</p>
            </div>
          </header>
          <div className="settings-body">
            <div className="setting-block">
              <div className="setting-label">
                <strong>Codex 会话目录</strong>
                <span>
                  默认仍会探测 ~/.codex/sessions；可添加其他实际路径。
                </span>
              </div>
              <div className="path-list">
                {settings.sessionPaths.map((path) => (
                  <div key={path}>
                    <PathText value={path} />
                    <button
                      className="icon-button danger"
                      aria-label={`删除 ${path}`}
                      onClick={() =>
                        setSettings({
                          ...settings,
                          sessionPaths: settings.sessionPaths.filter(
                            (item) => item !== path,
                          ),
                        })
                      }
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                ))}
              </div>
              <button
                className="button button-secondary"
                onClick={() => void addSessionPath()}
              >
                <FolderPlus size={14} />
                添加目录
              </button>
            </div>
            <div className="setting-block">
              <div className="setting-label">
                <strong>默认工作目录</strong>
                <span>
                  创建项目时的起始位置；每次续接仍使用项目自己的显式目录。
                </span>
              </div>
              <button
                className="path-picker"
                onClick={async () => {
                  const path = await choosePath("选择默认工作目录");
                  if (path)
                    setSettings({ ...settings, defaultWorkingDirectory: path });
                }}
              >
                <PathText
                  value={settings.defaultWorkingDirectory}
                  empty="尚未设置"
                />
                <span>选择</span>
              </button>
            </div>
          </div>
        </section>
        <section className="settings-section">
          <header>
            <Waypoints size={18} />
            <div>
              <h2>上下文编译</h2>
              <p>RuleBasedProvider 的默认预算和保留策略。</p>
            </div>
          </header>
          <div className="settings-body">
            <div className="setting-row">
              <label>
                <span>默认上下文预算</span>
                <input
                  type="number"
                  min={1000}
                  step={1000}
                  value={settings.defaultContextBudget}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      defaultContextBudget: Number(event.target.value),
                    })
                  }
                />
              </label>
              <label>
                <span>压缩策略</span>
                <select
                  value={settings.compressionStrategy}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      compressionStrategy: event.target
                        .value as AppSettings["compressionStrategy"],
                    })
                  }
                >
                  <option value="conservative">保守</option>
                  <option value="balanced">平衡</option>
                  <option value="aggressive">激进</option>
                </select>
              </label>
            </div>
            <div className="setting-row">
              <label><span>最近消息数量</span><input type="number" min={1} max={500} value={settings.recentMessageLimit} onChange={(event) => setSettings({ ...settings, recentMessageLimit: Number(event.target.value) })} /></label>
              <label><span>工具输出最大长度</span><input type="number" min={100} value={settings.toolOutputMaxLength} onChange={(event) => setSettings({ ...settings, toolOutputMaxLength: Number(event.target.value) })} /></label>
            </div>
            <p className="settings-note">
              Token 为确定性字符估算。Continuum
              不会声称可以精确测量模型能力下降。
            </p>
          </div>
        </section>
        <section className="settings-section">
          <header>
            <Bot size={18} />
            <div>
              <h2>Agent 启动命令</h2>
              <p>
                Fresh Continuation 创建全新进程；Resume 与 Fork
                始终作为单独操作。
              </p>
            </div>
          </header>
          <div className="settings-body">
            <div className="form-grid two-cols">
              <label>
                <span>Codex 启动命令</span>
                <input
                  value={settings.codexCommand}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      codexCommand: event.target.value,
                    })
                  }
                />
              </label>
              <label>
                <span>Claude Code 启动命令</span>
                <input
                  value={settings.claudeCommand}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      claudeCommand: event.target.value,
                    })
                  }
                />
              </label>
              <label>
                <span>终端程序</span>
                <input
                  value={settings.terminalProgram}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      terminalProgram: event.target.value,
                    })
                  }
                />
              </label>
              <label>
                <span>Codex 安装路径（可选）</span>
                <input
                  value={settings.agentInstallPaths.codex ?? ""}
                  onChange={(event) =>
                    setSettings({
                      ...settings,
                      agentInstallPaths: {
                        ...settings.agentInstallPaths,
                        codex: event.target.value,
                      },
                    })
                  }
                />
              </label>
            </div>
          </div>
        </section>
        <section className="settings-section">
          <header>
            <Settings2 size={18} />
            <div>
              <h2>扫描与监听</h2>
              <p>控制来源会话的增量同步范围。</p>
            </div>
          </header>
          <div className="settings-body toggle-stack">
            <div className="setting-row full-setting-row"><label><span>自动扫描间隔（秒）</span><input type="number" min={2} max={3600} value={settings.autoScanIntervalSeconds} onChange={(event) => setSettings({ ...settings, autoScanIntervalSeconds: Number(event.target.value) })} /></label><label><span>日志级别</span><select value={settings.logLevel} onChange={(event) => setSettings({ ...settings, logLevel: event.target.value as AppSettings["logLevel"] })}><option value="error">error</option><option value="warn">warn</option><option value="info">info</option><option value="debug">debug</option></select></label></div>
            <Toggle
              checked={settings.autoScan}
              onChange={(value) =>
                setSettings({ ...settings, autoScan: value })
              }
              label="启动时自动扫描"
              detail="索引已配置的真实会话目录"
            />
            <Toggle
              checked={settings.autoWatch}
              onChange={(value) =>
                setSettings({ ...settings, autoWatch: value })
              }
              label="自动监听新消息"
              detail="绑定后轮询来源文件并增量导入"
            />
            <Toggle
              checked={settings.collectCommandLogs}
              onChange={(value) =>
                setSettings({ ...settings, collectCommandLogs: value })
              }
              label="保存工具日志"
              detail="超长输出由 Context Compiler 压缩"
            />
            <Toggle
              checked={settings.saveModelThoughts}
              onChange={(value) =>
                setSettings({ ...settings, saveModelThoughts: value })
              }
              label="保存模型思考内容"
              detail="默认关闭；仅在来源格式明确提供时读取"
            />
            <Toggle
              checked={settings.readGitState}
              onChange={(value) =>
                setSettings({ ...settings, readGitState: value })
              }
              label="读取 Git 状态"
              detail="只执行带超时的只读命令"
            />
            <Toggle
              checked={settings.securityScan}
              onChange={(value) =>
                setSettings({ ...settings, securityScan: value })
              }
              label="敏感信息处理"
              detail="Raw Data 与上下文写入前脱敏"
            />
            <Toggle checked={settings.runInBackground} onChange={(value) => setSettings({ ...settings, runInBackground: value })} label="允许后台运行" detail="关闭窗口后的具体托盘行为由系统支持状态决定" />
          </div>
        </section>
        <section className="settings-section system-info">
          <header>
            <Database size={18} />
            <div>
              <h2>本地存储与安全边界</h2>
              <p>项目图、Context Snapshot 和续接绑定持久化在 SQLite。</p>
            </div>
          </header>
          <div className="settings-body">
            <div className="database-path">
              <span>SQLite 数据库</span>
              <PathText value={settings.databasePath} />
            </div>
            <div className="setting-row">
              <label><span>备份目录</span><div className="inline-input"><input value={settings.backupDirectory} onChange={(event) => setSettings({ ...settings, backupDirectory: event.target.value })} placeholder="留空使用应用数据目录" /><button className="button button-secondary" onClick={async () => { const path = await choosePath("选择数据库备份目录"); if (path) setSettings({ ...settings, backupDirectory: path }); }}>选择</button></div></label>
              <label><span>主题</span><select value={settings.theme} onChange={(event) => setSettings({ ...settings, theme: event.target.value as AppSettings["theme"] })}><option value="dark">深色</option><option value="system">跟随系统</option></select></label>
            </div>
            <div className="setting-row"><label><span>健康提醒阈值</span><input type="number" min={0.1} max={2} step={0.05} value={settings.healthWarningRatio} onChange={(event) => setSettings({ ...settings, healthWarningRatio: Number(event.target.value) })} /></label><label><span>严重提醒阈值</span><input type="number" min={0.1} max={2} step={0.05} value={settings.healthCriticalRatio} onChange={(event) => setSettings({ ...settings, healthCriticalRatio: Number(event.target.value) })} /></label></div>
            <div className="privacy-boundary">
              <ShieldCheck size={18} />
              <div>
                <strong>本地优先</strong>
                <p>
                  不上传会话、不自动运行历史命令、不改写第三方会话文件。Fresh
                  Continuation 只在用户点击后启动全新 Agent 进程。
                </p>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
