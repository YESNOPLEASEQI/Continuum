import { AlertTriangle, Check, LoaderCircle, X } from "lucide-react";
import type { ReactNode } from "react";

export function PageHeader({ eyebrow, title, description, actions }: { eyebrow: string; title: string; description: string; actions?: ReactNode }) {
  return (
    <header className="page-header">
      <div>
        <p className="eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        <p className="page-description">{description}</p>
      </div>
      {actions && <div className="page-actions">{actions}</div>}
    </header>
  );
}

export function Badge({ tone = "neutral", children }: { tone?: "neutral" | "signal" | "success" | "warning" | "danger"; children: ReactNode }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function PathText({ value, empty = "未记录" }: { value: string | null | undefined; empty?: string }) {
  if (!value) return <span className="text-muted">{empty}</span>;
  return <span className="path-text" title={value}>{value}</span>;
}

export function LoadingState({ label = "正在读取本地数据" }: { label?: string }) {
  return (
    <div className="state-panel" role="status" aria-live="polite">
      <LoaderCircle className="animate-spin text-signal" size={22} />
      <div><strong>{label}</strong><p>数据始终留在这台设备上。</p></div>
    </div>
  );
}

export function EmptyState({ icon, title, detail, action }: { icon: ReactNode; title: string; detail: string; action?: ReactNode }) {
  return (
    <div className="empty-state">
      <div className="empty-icon" aria-hidden="true">{icon}</div>
      <div><h2>{title}</h2><p>{detail}</p>{action && <div className="empty-action">{action}</div>}</div>
    </div>
  );
}

export function ErrorState({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="state-panel state-error" role="alert">
      <AlertTriangle size={21} />
      <div><strong>本地操作未完成</strong><p>{message}</p></div>
      {onRetry && <button className="button button-secondary" onClick={onRetry}>重试</button>}
    </div>
  );
}

export function ConfirmDialog({ open, title, description, confirmLabel, destructive = false, onConfirm, onCancel }: {
  open: boolean; title: string; description: string; confirmLabel: string; destructive?: boolean; onConfirm: () => void; onCancel: () => void;
}) {
  if (!open) return null;
  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onCancel}>
      <div className="dialog" role="alertdialog" aria-modal="true" aria-labelledby="dialog-title" onMouseDown={(event) => event.stopPropagation()}>
        <div className="dialog-mark"><AlertTriangle size={20} /></div>
        <h2 id="dialog-title">{title}</h2>
        <p>{description}</p>
        <div className="dialog-actions">
          <button className="button button-secondary" onClick={onCancel}>取消</button>
          <button autoFocus className={`button ${destructive ? "button-danger" : "button-primary"}`} onClick={onConfirm}>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}

export function Toggle({ checked, onChange, label, detail }: { checked: boolean; onChange: (checked: boolean) => void; label: string; detail: string }) {
  return (
    <label className="toggle-row">
      <span><strong>{label}</strong><small>{detail}</small></span>
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      <span className="toggle-control" aria-hidden="true"><span /></span>
    </label>
  );
}

export function ToastIcon({ tone }: { tone: "success" | "error" | "info" }) {
  return tone === "success" ? <Check size={17} /> : tone === "error" ? <AlertTriangle size={17} /> : <span className="toast-dot" />;
}

export function CloseIcon() {
  return <X size={15} />;
}
