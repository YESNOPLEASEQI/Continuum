import { create } from "zustand";
import { appApi } from "../api/bridge";
import type { DashboardStats, PackageSummary, SessionSummary, UnifiedProjectSummary } from "../types/models";

interface ToastMessage {
  id: number;
  tone: "success" | "error" | "info";
  title: string;
  detail?: string;
}

interface AppStore {
  dashboard: DashboardStats | null;
  sessions: SessionSummary[];
  packages: PackageSummary[];
  projects: UnifiedProjectSummary[];
  loading: boolean;
  scanning: boolean;
  error: string | null;
  toasts: ToastMessage[];
  timelineRevision: number;
  watcherErrorCount: number;
  loadDashboard: () => Promise<void>;
  loadSessions: () => Promise<void>;
  scanSessions: () => Promise<void>;
  loadPackages: () => Promise<void>;
  loadProjects: () => Promise<void>;
  pollChanges: () => Promise<void>;
  notify: (toast: Omit<ToastMessage, "id">) => void;
  dismissToast: (id: number) => void;
  clearError: () => void;
}

let toastSequence = 0;
let pollInFlight = false;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const useAppStore = create<AppStore>((set, get) => ({
  dashboard: null,
  sessions: [],
  packages: [],
  projects: [],
  loading: false,
  scanning: false,
  error: null,
  toasts: [],
  timelineRevision: 0,
  watcherErrorCount: 0,
  loadDashboard: async () => {
    set({ loading: true, error: null });
    try {
      set({ dashboard: await appApi.dashboard(), loading: false });
    } catch (error) {
      set({ error: errorMessage(error), loading: false });
    }
  },
  loadSessions: async () => {
    set({ loading: true, error: null });
    try {
      set({ sessions: await appApi.sessions(), loading: false });
    } catch (error) {
      set({ error: errorMessage(error), loading: false });
    }
  },
  scanSessions: async () => {
    set({ scanning: true, error: null });
    try {
      const sessions = await appApi.scan();
      set({ sessions, scanning: false });
      get().notify({ tone: "success", title: "扫描完成", detail: `发现 ${sessions.length} 个 Codex 会话` });
      await get().loadDashboard();
    } catch (error) {
      const message = errorMessage(error);
      set({ error: message, scanning: false });
      get().notify({ tone: "error", title: "扫描失败", detail: message });
    }
  },
  loadPackages: async () => {
    set({ loading: true, error: null });
    try {
      set({ packages: await appApi.packages(), loading: false });
    } catch (error) {
      set({ error: errorMessage(error), loading: false });
    }
  },
  loadProjects: async () => {
    set({ loading: true, error: null });
    try {
      set({ projects: await appApi.projects(), loading: false });
    } catch (error) {
      set({ error: errorMessage(error), loading: false });
    }
  },
  pollChanges: async () => {
    if (pollInFlight) return;
    pollInFlight = true;
    try {
      const result = await appApi.pollSessionChanges();
      if (result.insertedNodes > 0 || result.newSessions > 0) {
        set((state) => ({ timelineRevision: state.timelineRevision + 1, watcherErrorCount: result.parseErrors }));
        await Promise.all([get().loadDashboard(), get().loadSessions()]);
      } else if (result.parseErrors !== get().watcherErrorCount) {
        set({ watcherErrorCount: result.parseErrors });
      }
    } catch {
      // Transient file locks and startup races are retried on the next watcher tick.
    } finally {
      pollInFlight = false;
    }
  },
  notify: (toast) => {
    const id = ++toastSequence;
    set((state) => ({ toasts: [...state.toasts, { ...toast, id }] }));
    window.setTimeout(() => get().dismissToast(id), 4200);
  },
  dismissToast: (id) => set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) })),
  clearError: () => set({ error: null }),
}));
