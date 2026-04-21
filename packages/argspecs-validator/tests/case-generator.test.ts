import { describe, test, expect } from "bun:test";
import { generateCases } from "../src/generate/case-generator.js";
import type { TestRecord } from "../src/types.js";

describe("generateCases", () => {
  test("\\sqrt (spec: o m) → baseline + vary:o + bare", () => {
    const record: TestRecord = {
      package: "base", name: "sqrt", type: "command",
      argspec: "o m", kind: "prefix", allowed_mode: "math", tags: [],
    };
    const cases = generateCases(record);
    const branches = cases.map((c) => c.branch);
    expect(branches).toContain("baseline");
    expect(branches).toContain("vary:o[0]");
    expect(branches).toContain("bare[1]");
    expect(branches.filter((b) => b === "maximal")).toHaveLength(0);
  });

  test("\\textbf (spec: m:T) → baseline + bare + neg:T", () => {
    const record: TestRecord = {
      package: "textmacros", name: "textbf", type: "command",
      argspec: "m:T", kind: "prefix", allowed_mode: "text", tags: [],
    };
    const cases = generateCases(record);
    const branches = cases.map((c) => c.branch);
    expect(branches).toContain("baseline");
    expect(branches).toContain("bare[0]");
    expect(branches).toContain("neg:T[0]");
    const neg = cases.find((c) => c.branch === "neg:T[0]")!;
    expect(neg.positive).toBe(false);
    expect(neg.expect).toBe("fail");
  });

  test("empty spec → single baseline", () => {
    const record: TestRecord = {
      package: "base", name: "arccos", type: "command",
      argspec: "", kind: "prefix", allowed_mode: "math", tags: [],
    };
    const cases = generateCases(record);
    expect(cases).toHaveLength(1);
    expect(cases[0].branch).toBe("baseline");
    expect(cases[0].tex).toBe("\\arccos");
  });

  test("environment with matrix tag", () => {
    const record: TestRecord = {
      package: "ams", name: "Bmatrix", type: "environment",
      argspec: "", allowed_mode: "math", tags: ["matrix", "nestable"],
    };
    const cases = generateCases(record);
    expect(cases).toHaveLength(1);
    expect(cases[0].tex).toContain("\\begin{Bmatrix}");
    expect(cases[0].tex).toContain("a & b");
  });

  test("drops later case when two optional branches generate the same tex", () => {
    const record: TestRecord = {
      package: "physics", name: "ev", type: "command",
      argspec: "s s m", kind: "prefix", allowed_mode: "math", tags: [],
    };
    const cases = generateCases(record);

    expect(cases.map((c) => c.branch)).toEqual([
      "baseline",
      "vary:s[0]",
      "maximal",
      "bare[2]",
    ]);
    expect(cases.filter((c) => c.tex === "\\ev*{a}")).toHaveLength(1);
  });

  test("drops later case when a bare branch duplicates baseline tex", () => {
    const record: TestRecord = {
      package: "base", name: "above", type: "command",
      argspec: "m:L", kind: "infix", allowed_mode: "math", tags: [],
    };
    const cases = generateCases(record);

    expect(cases.map((c) => c.branch)).toEqual([
      "baseline",
      "neg:L[0]",
    ]);
    expect(cases.filter((c) => c.tex === "a \\above 1pt b")).toHaveLength(1);
  });
});
