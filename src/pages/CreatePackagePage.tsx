import { ArrowLeft, CheckCircle2, PackagePlus, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { useNavigate, useSearchParams } from "react-router-dom";
import { appApi } from "../api/bridge";
import { ErrorState, LoadingState, PageHeader, Toggle } from "../components/ui";
import { useAppStore } from "../store/appStore";
import type { PackageDraft } from "../types/models";
import { packageDraftSchema } from "../types/schemas";

const fallbackDraft: PackageDraft = { sourceSessionId: "", title: "", originalGoal: "", currentState: "", completedWork: "", remainingWork: "", nextActions: "", decisions: "", knownIssues: "", failedAttempts: "", constraints: "", requiredTools: "git\nshell\nfilesystem", targetAgent: "codex", includeGit: true, includePatch: true, includeUntracked: false, includeTests: true, includeCommandLog: true };

const textAreas: Array<{ name: keyof Pick<PackageDraft, "originalGoal" | "currentState" | "completedWork" | "remainingWork" | "nextActions" | "decisions" | "knownIssues" | "failedAttempts" | "constraints" | "requiredTools">; label: string; hint: string; required?: boolean }> = [
  { name: "originalGoal", label: "原始目标", hint: "用户最初希望完成什么", required: true },
  { name: "currentState", label: "当前状态", hint: "任务现在停在哪里", required: true },
  { name: "completedWork", label: "已完成工作", hint: "每行一项" },
  { name: "remainingWork", label: "未完成工作", hint: "仍待处理的范围" },
  { name: "nextActions", label: "下一步操作", hint: "按优先级每行一项", required: true },
  { name: "decisions", label: "关键决策", hint: "需要保留的技术或产品决策" },
  { name: "knownIssues", label: "已知问题", hint: "缺陷、阻塞或不确定项" },
  { name: "failedAttempts", label: "失败尝试", hint: "不要让下一个 Agent 重复的操作" },
  { name: "constraints", label: "不可违反的约束", hint: "安全、范围与兼容性边界" },
  { name: "requiredTools", label: "所需工具", hint: "每行一个工具或能力" },
];

export default function CreatePackagePage() {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const sessionId = params.get("session") ?? "";
  const { sessions, loadSessions, notify } = useAppStore();
  const [prefilling, setPrefilling] = useState(Boolean(sessionId));
  const [loadError, setLoadError] = useState<string | null>(null);
  const form = useForm<PackageDraft>({ resolver: zodResolver(packageDraftSchema), defaultValues: { ...fallbackDraft, sourceSessionId: sessionId } });
  const { register, handleSubmit, reset, control, formState: { errors, isSubmitting } } = form;

  useEffect(() => { void loadSessions(); }, [loadSessions]);
  useEffect(() => {
    if (!sessionId) return;
    appApi.packageDraft(sessionId).then((draft) => reset(draft)).catch((reason: unknown) => setLoadError(reason instanceof Error ? reason.message : String(reason))).finally(() => setPrefilling(false));
  }, [sessionId, reset]);

  const submit = handleSubmit(async (draft) => {
    try {
      const created = await appApi.createPackage(draft);
      notify({ tone: "success", title: "任务包已创建", detail: created.packagePath });
      navigate(`/packages/${created.id}`);
    } catch (reason) {
      notify({ tone: "error", title: "创建失败", detail: reason instanceof Error ? reason.message : String(reason) });
    }
  });

  if (prefilling) return <LoadingState label="正在从会话提取任务状态" />;
  if (loadError) return <div className="page"><ErrorState message={loadError} onRetry={() => navigate("/sessions")} /></div>;

  return <div className="page package-form-page">
    <button className="back-button" onClick={() => navigate(-1)}><ArrowLeft size={15} />返回</button>
    <PageHeader eyebrow="PACKAGE BUILDER" title="创建任务接力包" description="内容由规则从真实会话预填；写入磁盘前可以逐项修订。" />
    <form onSubmit={submit} noValidate className="package-form">
      <section className="form-main">
        <div className="form-section-heading"><span>01</span><div><h2>任务身份</h2><p>确定来源和恢复目标。</p></div></div>
        <div className="form-grid two-cols">
          <label><span>来源会话</span><select {...register("sourceSessionId")} aria-invalid={Boolean(errors.sourceSessionId)}><option value="">请选择会话</option>{sessions.map((session) => <option value={session.id} key={session.id}>{session.title} · {session.id.slice(0, 8)}</option>)}</select>{errors.sourceSessionId && <small role="alert" className="field-error">{errors.sourceSessionId.message}</small>}</label>
          <label><span>推荐目标 Agent</span><select {...register("targetAgent")}><option value="codex">Codex CLI</option><option value="claude">Claude Code（兼容预览）</option><option value="gemini">Gemini CLI（兼容预览）</option><option value="opencode">OpenCode（兼容预览）</option></select></label>
          <label className="full-field"><span>任务标题</span><input {...register("title")} placeholder="例：完成任务包导入校验" aria-invalid={Boolean(errors.title)} />{errors.title && <small role="alert" className="field-error">{errors.title.message}</small>}</label>
        </div>
        <div className="form-section-heading"><span>02</span><div><h2>交接内容</h2><p>使用明确、可验证的句子；列表字段每行一项。</p></div></div>
        <div className="form-grid two-cols text-area-grid">{textAreas.map((field) => <label key={field.name} className={field.name === "originalGoal" || field.name === "currentState" ? "full-field" : ""}><span>{field.label}{field.required && <em>*</em>}</span><small>{field.hint}</small><textarea rows={field.name === "originalGoal" || field.name === "currentState" ? 4 : 5} {...register(field.name)} aria-invalid={Boolean(errors[field.name])} />{errors[field.name] && <small role="alert" className="field-error">{errors[field.name]?.message}</small>}</label>)}</div>
      </section>
      <aside className="form-options">
        <p className="eyebrow">INCLUDED EVIDENCE</p><h2>附带证据</h2>
        <Controller name="includeGit" control={control} render={({ field }) => <Toggle checked={field.value} onChange={field.onChange} label="Git 信息" detail="分支、HEAD 与状态" />} />
        <Controller name="includePatch" control={control} render={({ field }) => <Toggle checked={field.value} onChange={field.onChange} label="未提交补丁" detail="只读取 working tree diff" />} />
        <Controller name="includeUntracked" control={control} render={({ field }) => <Toggle checked={field.value} onChange={field.onChange} label="未跟踪文件列表" detail="不复制文件内容" />} />
        <Controller name="includeTests" control={control} render={({ field }) => <Toggle checked={field.value} onChange={field.onChange} label="测试结果" detail="仅保存会话中的已有证据" />} />
        <Controller name="includeCommandLog" control={control} render={({ field }) => <Toggle checked={field.value} onChange={field.onChange} label="命令日志" detail="绝不会自动重新执行" />} />
        <div className="security-callout"><ShieldCheck size={18} /><div><strong>写入前自动脱敏</strong><p>检测到的令牌会替换为 [REDACTED]，原文不会进入任务包。</p></div></div>
        <button className="button button-primary button-wide" disabled={isSubmitting} data-testid="create-package-submit"><PackagePlus size={16} />{isSubmitting ? "正在生成…" : "生成任务包"}</button>
        <div className="builder-check"><CheckCircle2 size={14} />JSON、哈希、文件结构均在后端校验</div>
      </aside>
    </form>
  </div>;
}
