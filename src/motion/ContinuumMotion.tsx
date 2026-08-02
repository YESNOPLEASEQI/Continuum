import { useGSAP } from "@gsap/react";
import gsap from "gsap";
import { CustomEase } from "gsap/CustomEase";
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useNavigate } from "react-router-dom";

gsap.registerPlugin(useGSAP, CustomEase);

const blockEase = CustomEase.create(
  "continuumBlockEase",
  "M0,0 C0.22,1 0.36,1 1,1",
);
const ROWS = 10;
const COLS = 11;
const BLOCKS = Array.from({ length: ROWS * COLS }, (_, index) => index);

interface MajorOperationState {
  label: string;
  phase: "covering" | "running" | "complete" | "failed";
  detail: string;
}

interface ContinuumMotionValue {
  navigateMajor: (to: string, label: string) => Promise<void>;
  runMajorOperation: <T>(
    label: string,
    operation: () => Promise<T>,
  ) => Promise<T | undefined>;
  operation: MajorOperationState | null;
}

const ContinuumMotionContext = createContext<ContinuumMotionValue | null>(null);

function prefersReducedMotion() {
  return (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches
  );
}

function blockDelay(index: number) {
  const row = Math.floor(index / COLS);
  const column = index % COLS;
  const rowDelay = (ROWS - row - 1) * 0.016;
  const deterministicJitter = ((column * 7 + row * 3) % 11) * 0.008;
  return rowDelay + deterministicJitter;
}

export function ContinuumMotionProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const layerRef = useRef<HTMLDivElement>(null);
  const lockedRef = useRef(false);
  const [operation, setOperation] = useState<MajorOperationState | null>(null);

  const { contextSafe } = useGSAP(
    () => {
      gsap.set(".continuum-transition-in .continuum-transition-block", {
        scaleY: 0,
        transformOrigin: "top",
      });
      gsap.set(".continuum-transition-out .continuum-transition-block", {
        scaleY: 0,
        transformOrigin: "bottom",
      });
    },
    { scope: layerRef },
  );

  const animateBlocks = useCallback(
    (selector: string, scaleY: number) =>
      new Promise<void>((resolve) => {
        const targets = layerRef.current?.querySelectorAll(selector);
        if (!targets?.length || prefersReducedMotion()) {
          resolve();
          return;
        }
        gsap.to(targets, {
          scaleY,
          duration: 0.72,
          ease: blockEase,
          stagger: blockDelay,
          overwrite: true,
          onComplete: resolve,
        });
      }),
    [],
  );

  const runMajorOperation = contextSafe(
    async <T,>(label: string, task: () => Promise<T>) => {
      if (lockedRef.current) return undefined;
      lockedRef.current = true;
      const reduced = prefersReducedMotion();
      let revealed = reduced;
      setOperation({
        label,
        phase: reduced ? "running" : "covering",
        detail: reduced ? "正在执行本地操作" : "正在封存当前视图",
      });
      try {
        if (!reduced) {
          gsap.set(
            ".continuum-transition-out .continuum-transition-block",
            { scaleY: 0, transformOrigin: "bottom" },
          );
          await animateBlocks(
            ".continuum-transition-out .continuum-transition-block",
            1,
          );
        }
        setOperation({ label, phase: "running", detail: "正在执行本地操作" });
        const outcome = task().then(
          (value) => ({ ok: true as const, value }),
          (reason: unknown) => ({ ok: false as const, reason }),
        );
        if (!reduced) {
          gsap.set(".continuum-transition-in .continuum-transition-block", {
            scaleY: 1,
            transformOrigin: "top",
          });
          gsap.set(".continuum-transition-out .continuum-transition-block", {
            scaleY: 0,
          });
          await animateBlocks(
            ".continuum-transition-in .continuum-transition-block",
            0,
          );
          revealed = true;
        }
        const settled = await outcome;
        if (!settled.ok) throw settled.reason;
        setOperation({ label, phase: "complete", detail: "操作已完成" });
        return settled.value;
      } catch (reason) {
        setOperation({
          label,
          phase: "failed",
          detail: reason instanceof Error ? reason.message : String(reason),
        });
        if (!reduced && !revealed) {
          gsap.set(".continuum-transition-in .continuum-transition-block", {
            scaleY: 1,
            transformOrigin: "top",
          });
          gsap.set(".continuum-transition-out .continuum-transition-block", {
            scaleY: 0,
          });
          await animateBlocks(
            ".continuum-transition-in .continuum-transition-block",
            0,
          );
        }
        throw reason;
      } finally {
        lockedRef.current = false;
        setOperation(null);
      }
    },
  );

  const navigateMajor = useCallback(
    async (to: string, label: string) => {
      await runMajorOperation(label, async () => navigate(to));
    },
    [navigate, runMajorOperation],
  );

  const value = useMemo<ContinuumMotionValue>(
    () => ({ navigateMajor, runMajorOperation, operation }),
    [navigateMajor, operation, runMajorOperation],
  );

  return (
    <ContinuumMotionContext.Provider value={value}>
      {children}
      <div
        ref={layerRef}
        className={`continuum-transition-layer ${operation ? `is-active is-${operation.phase}` : ""}`}
        aria-hidden={!operation}
      >
        <div className="continuum-transition-grid continuum-transition-in">
          {BLOCKS.map((index) => (
            <i className="continuum-transition-block" key={`in-${index}`} />
          ))}
        </div>
        <div className="continuum-transition-grid continuum-transition-out">
          {BLOCKS.map((index) => (
            <i className="continuum-transition-block" key={`out-${index}`} />
          ))}
        </div>
        {operation && (
          <div className="continuum-operation" role="status" aria-live="polite">
            <span>{operation.phase}</span>
            <strong>{operation.label}</strong>
            <p>{operation.detail}</p>
          </div>
        )}
      </div>
    </ContinuumMotionContext.Provider>
  );
}

export function useContinuumMotion() {
  const context = useContext(ContinuumMotionContext);
  if (!context) {
    throw new Error("useContinuumMotion must be used inside ContinuumMotionProvider");
  }
  return context;
}
