import { useCallback } from "react";
import { useAppStore } from "../store/appStore";
import { useContinuumMotion } from "./ContinuumMotion";

export function useMajorSessionScan() {
  const scanSessions = useAppStore((state) => state.scanSessions);
  const { runMajorOperation } = useContinuumMotion();

  return useCallback(async () => {
    try {
      await runMajorOperation("扫描 Codex 会话", async () => {
        await scanSessions();
        const scanError = useAppStore.getState().error;
        if (scanError) throw new Error(scanError);
      });
    } catch {
      // The store already exposes the backend failure through its toast and error state.
    }
  }, [runMajorOperation, scanSessions]);
}
