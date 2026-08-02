import { describe, expect, it } from "vitest";
import { calculateCompatibility } from "../src/config/agents";

describe("compatibility calculation", () => {
  it("scores identical agents at 100", () => expect(calculateCompatibility("codex", "codex").score).toBe(100));
  it("reports target tool gaps", () => {
    const result = calculateCompatibility("codex", "cursor");
    expect(result.missingTools).toContain("shell");
    expect(result.score).toBeLessThan(100);
  });
});
