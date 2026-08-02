import {
  ExternalLink,
  FileWarning,
  KeyRound,
  ListChecks,
  LoaderCircle,
  MessageSquareText,
  ShieldAlert,
  Terminal,
  Wifi,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { appApi } from "../api/bridge";
import { useAppStore } from "../store/appStore";
import type {
  AppServerApprovalDecision,
  AppServerClientRequest,
  AppServerClientResponse,
} from "../types/models";

type JsonSchema = {
  type?: string;
  title?: string;
  description?: string;
  format?: string;
  default?: unknown;
  enum?: string[];
  enumNames?: string[];
  oneOf?: Array<{ const?: string; title?: string }>;
  items?: JsonSchema;
  properties?: Record<string, JsonSchema>;
  required?: string[];
  minimum?: number;
  maximum?: number;
};

const kindLabels: Record<AppServerClientRequest["kind"], string> = {
  command: "命令执行",
  file_change: "文件修改",
  network: "网络访问",
  permissions: "权限申请",
  mcp_elicitation: "MCP 请求",
  tool_user_input: "需要回答",
};

function schemaOf(value: Record<string, unknown> | null): JsonSchema {
  return (value ?? {}) as JsonSchema;
}

function initialFormValues(schema: JsonSchema) {
  return Object.fromEntries(
    Object.entries(schema.properties ?? {}).map(([key, field]) => [
      key,
      field.default ?? (field.type === "boolean" ? false : field.type === "array" ? [] : ""),
    ]),
  );
}

function hasValue(value: unknown) {
  return value !== undefined && value !== null && value !== "";
}

function safeExternalUrl(value: string | null) {
  if (!value) return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:" ? url.toString() : null;
  } catch {
    return null;
  }
}

export function AppServerApprovalDialog() {
  const notify = useAppStore((state) => state.notify);
  const [requests, setRequests] = useState<AppServerClientRequest[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [formValues, setFormValues] = useState<Record<string, unknown>>({});
  const [rawFormValue, setRawFormValue] = useState("{}");
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const pollingRef = useRef(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const safeActionRef = useRef<HTMLButtonElement>(null);
  const current = requests[0];

  const refresh = useCallback(async () => {
    if (pollingRef.current) return;
    pollingRef.current = true;
    try {
      setRequests(await appApi.appServerRequests());
    } catch {
      // The next poll retries transient startup or IPC failures.
    } finally {
      pollingRef.current = false;
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 800);
    return () => window.clearInterval(timer);
  }, [refresh]);

  useEffect(() => {
    if (!current) return;
    setError(null);
    setFormValues(initialFormValues(schemaOf(current.requestedSchema)));
    setRawFormValue("{}");
    setAnswers({});
    safeActionRef.current?.focus();
    const trapFocus = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || !dialogRef.current) return;
      const focusable = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(
          "button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href]",
        ),
      );
      if (focusable.length === 0) return;
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
    document.addEventListener("keydown", trapFocus);
    return () => document.removeEventListener("keydown", trapFocus);
  }, [current?.id]);

  async function respond(response: AppServerClientResponse, accepted: boolean) {
    if (!current) return;
    setBusy(true);
    setError(null);
    try {
      await appApi.respondAppServerRequest(current.id, response);
      setRequests((items) => items.filter((item) => item.id !== current.id));
      notify({
        tone: accepted ? "success" : "info",
        title: accepted ? "请求已响应" : "请求已拒绝",
        detail: kindLabels[current.kind],
      });
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  function respondToApproval(decision: AppServerApprovalDecision) {
    return respond({ decision }, decision.startsWith("accept"));
  }

  function submitMcpForm() {
    if (!current) return;
    const schema = schemaOf(current.requestedSchema);
    let content: Record<string, unknown>;
    if (schema.properties) {
      const missing = (schema.required ?? []).find((key) => !hasValue(formValues[key]));
      if (missing) {
        setError(`请填写必填字段：${schema.properties[missing]?.title ?? missing}`);
        return;
      }
      content = formValues;
    } else {
      try {
        const parsed = JSON.parse(rawFormValue) as unknown;
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
          throw new Error("表单内容必须是 JSON 对象");
        }
        content = parsed as Record<string, unknown>;
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : "JSON 格式无效");
        return;
      }
    }
    void respond({ action: "accept", content }, true);
  }

  function submitToolAnswers() {
    if (!current) return;
    const unanswered = current.questions.find((question) => !answers[question.id]?.trim());
    if (unanswered) {
      setError(`请回答：${unanswered.header || unanswered.question}`);
      return;
    }
    void respond(
      {
        answers: Object.fromEntries(
          current.questions.map((question) => [
            question.id,
            { answers: [answers[question.id].trim()] },
          ]),
        ),
      },
      true,
    );
  }

  if (!current) return null;
  const externalUrl = safeExternalUrl(current.url);
  const Icon =
    current.kind === "network"
      ? Wifi
      : current.kind === "file_change"
        ? FileWarning
        : current.kind === "permissions"
          ? KeyRound
          : current.kind === "mcp_elicitation"
            ? MessageSquareText
            : current.kind === "tool_user_input"
              ? ListChecks
              : Terminal;

  return (
    <div className="dialog-backdrop approval-backdrop">
      <div
        ref={dialogRef}
        className="dialog approval-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="approval-title"
        aria-describedby="approval-description"
      >
        <header>
          <div className="dialog-mark approval-mark">
            <ShieldAlert size={18} />
          </div>
          <div>
            <p className="eyebrow">CODEX APP SERVER</p>
            <h2 id="approval-title">Codex 需要你的响应</h2>
          </div>
          {requests.length > 1 && <span>队列 {requests.length}</span>}
        </header>
        <div className="approval-kind">
          <Icon size={15} />
          <strong>{kindLabels[current.kind]}</strong>
        </div>
        <p id="approval-description">
          {current.message ?? current.reason ?? "Codex 正在等待你的输入后继续。"}
        </p>

        <dl className="approval-details">
          {current.serverName && (
            <div>
              <dt>MCP 服务</dt>
              <dd><code>{current.serverName}</code></dd>
            </div>
          )}
          {current.networkHost && (
            <div>
              <dt>目标</dt>
              <dd><code>{current.networkProtocol}://{current.networkHost}</code></dd>
            </div>
          )}
          {current.command && (
            <div>
              <dt>命令</dt>
              <dd><pre>{current.command}</pre></dd>
            </div>
          )}
          {current.cwd && (
            <div>
              <dt>工作目录</dt>
              <dd><code>{current.cwd}</code></dd>
            </div>
          )}
          {current.grantRoot && (
            <div>
              <dt>写入范围</dt>
              <dd><code>{current.grantRoot}</code></dd>
            </div>
          )}
        </dl>

        {current.kind === "permissions" && (
          <section className="approval-request-body">
            <h3>申请的权限</h3>
            <pre>{JSON.stringify(current.permissions ?? {}, null, 2)}</pre>
          </section>
        )}

        {current.kind === "mcp_elicitation" && current.mode === "url" && (
          <section className="approval-request-body">
            <p>请在服务提供的页面完成操作，然后返回此处确认。</p>
            {externalUrl ? (
              <a className="button button-secondary" href={externalUrl} target="_blank" rel="noreferrer">
                打开服务页面 <ExternalLink size={14} />
              </a>
            ) : current.url ? (
              <p className="approval-error">服务返回了不受支持的链接协议。</p>
            ) : null}
          </section>
        )}

        {current.kind === "mcp_elicitation" && current.mode !== "url" && (
          <McpForm
            schema={schemaOf(current.requestedSchema)}
            values={formValues}
            rawValue={rawFormValue}
            onRawChange={setRawFormValue}
            onChange={(key, value) => setFormValues((items) => ({ ...items, [key]: value }))}
          />
        )}

        {current.kind === "tool_user_input" && (
          <section className="approval-request-body tool-input-questions">
            {current.autoResolutionMs !== null && (
              <p className="muted">未回答时 Codex 可在 {Math.ceil(current.autoResolutionMs / 1000)} 秒后自动继续。</p>
            )}
            {current.questions.map((question) => (
              <fieldset key={question.id}>
                <legend>{question.header}</legend>
                <p>{question.question}</p>
                {question.options?.map((option) => (
                  <label className="approval-option" key={option.label}>
                    <input
                      type="radio"
                      name={`question-${question.id}`}
                      value={option.label}
                      checked={answers[question.id] === option.label}
                      onChange={() => setAnswers((items) => ({ ...items, [question.id]: option.label }))}
                    />
                    <span><strong>{option.label}</strong><small>{option.description}</small></span>
                  </label>
                ))}
                {(!question.options?.length || question.isOther) && (
                  <input
                    type={question.isSecret ? "password" : "text"}
                    aria-label={`${question.header}回答`}
                    placeholder={question.isOther ? "其他回答" : "输入回答"}
                    value={answers[question.id] ?? ""}
                    onChange={(event) => setAnswers((items) => ({ ...items, [question.id]: event.target.value }))}
                  />
                )}
              </fieldset>
            ))}
          </section>
        )}

        {error && <p className="approval-error" role="alert">{error}</p>}
        <div className="approval-actions">
          {(current.kind === "command" || current.kind === "file_change" || current.kind === "network") && (
            <>
              <button ref={safeActionRef} className="button button-secondary" onClick={() => void respondToApproval("decline")} disabled={busy}>拒绝本次</button>
              <button className="button button-secondary approval-cancel" onClick={() => void respondToApproval("cancel")} disabled={busy}>拒绝并停止</button>
              <button className="button button-secondary" onClick={() => void respondToApproval("acceptForSession")} disabled={busy}>本会话允许</button>
              <button className="button button-primary" onClick={() => void respondToApproval("accept")} disabled={busy}>
                {busy ? <LoaderCircle className="animate-spin" size={14} /> : null}允许本次
              </button>
            </>
          )}
          {current.kind === "permissions" && (
            <>
              <button ref={safeActionRef} className="button button-secondary" onClick={() => void respond({ permissions: {}, scope: "turn" }, false)} disabled={busy}>拒绝全部</button>
              <button className="button button-secondary" onClick={() => void respond({ permissions: current.permissions ?? {}, scope: "turn" }, true)} disabled={busy}>本回合允许</button>
              <button className="button button-primary" onClick={() => void respond({ permissions: current.permissions ?? {}, scope: "session" }, true)} disabled={busy}>
                {busy ? <LoaderCircle className="animate-spin" size={14} /> : null}本会话允许
              </button>
            </>
          )}
          {current.kind === "mcp_elicitation" && (
            <>
              <button ref={safeActionRef} className="button button-secondary" onClick={() => void respond({ action: "decline" }, false)} disabled={busy}>拒绝</button>
              <button className="button button-secondary approval-cancel" onClick={() => void respond({ action: "cancel" }, false)} disabled={busy}>取消操作</button>
              <button className="button button-primary" onClick={() => current.mode === "url" ? void respond({ action: "accept" }, true) : submitMcpForm()} disabled={busy}>
                {busy ? <LoaderCircle className="animate-spin" size={14} /> : null}{current.mode === "url" ? "已完成并继续" : "提交并继续"}
              </button>
            </>
          )}
          {current.kind === "tool_user_input" && (
            <button ref={safeActionRef} className="button button-primary" onClick={submitToolAnswers} disabled={busy}>
              {busy ? <LoaderCircle className="animate-spin" size={14} /> : null}提交回答
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function McpForm({
  schema,
  values,
  rawValue,
  onRawChange,
  onChange,
}: {
  schema: JsonSchema;
  values: Record<string, unknown>;
  rawValue: string;
  onRawChange: (value: string) => void;
  onChange: (key: string, value: unknown) => void;
}) {
  if (!schema.properties) {
    return (
      <section className="approval-request-body">
        <label className="approval-field">
          <span>结构化响应（JSON）</span>
          <textarea rows={6} value={rawValue} onChange={(event) => onRawChange(event.target.value)} />
        </label>
      </section>
    );
  }
  return (
    <section className="approval-request-body approval-form">
      {Object.entries(schema.properties).map(([key, field]) => {
        const label = `${field.title ?? key}${schema.required?.includes(key) ? " *" : ""}`;
        const options = field.oneOf?.map((item) => ({ value: item.const ?? "", label: item.title ?? item.const ?? "" }))
          ?? field.enum?.map((value, index) => ({ value, label: field.enumNames?.[index] ?? value }));
        if (field.type === "boolean") {
          return (
            <label className="approval-option" key={key}>
              <input type="checkbox" checked={Boolean(values[key])} onChange={(event) => onChange(key, event.target.checked)} />
              <span><strong>{label}</strong>{field.description && <small>{field.description}</small>}</span>
            </label>
          );
        }
        if (field.type === "array" && field.items?.enum) {
          const selected = Array.isArray(values[key]) ? values[key] as string[] : [];
          return (
            <fieldset key={key}>
              <legend>{label}</legend>
              {field.description && <p>{field.description}</p>}
              {field.items.enum.map((option) => (
                <label className="approval-option" key={option}>
                  <input type="checkbox" checked={selected.includes(option)} onChange={(event) => onChange(key, event.target.checked ? [...selected, option] : selected.filter((item) => item !== option))} />
                  <span>{option}</span>
                </label>
              ))}
            </fieldset>
          );
        }
        return (
          <label className="approval-field" key={key}>
            <span>{label}</span>
            {options ? (
              <select value={String(values[key] ?? "")} onChange={(event) => onChange(key, event.target.value)}>
                <option value="">请选择</option>
                {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
              </select>
            ) : (
              <input
                type={field.type === "number" || field.type === "integer" ? "number" : field.format === "date" ? "date" : field.format === "date-time" ? "datetime-local" : field.format === "email" ? "email" : field.format === "uri" ? "url" : "text"}
                min={field.minimum}
                max={field.maximum}
                value={String(values[key] ?? "")}
                onChange={(event) => onChange(key, field.type === "number" || field.type === "integer" ? (event.target.value === "" ? "" : Number(event.target.value)) : event.target.value)}
              />
            )}
            {field.description && <small>{field.description}</small>}
          </label>
        );
      })}
    </section>
  );
}
