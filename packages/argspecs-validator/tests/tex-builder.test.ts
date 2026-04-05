import { describe, test, expect } from "bun:test";
import { buildCommandTex, buildEnvironmentTex, wrapSlot } from "../src/generate/tex-builder.js";
import type { TestRecord } from "../src/types.js";

describe("wrapSlot", () => {
  test("standard required → {value}", () => {
    expect(wrapSlot("a", { type: "standard" }, true)).toBe("{a}");
  });
  test("standard optional → [value]", () => {
    expect(wrapSlot("a", { type: "standard" }, false)).toBe("[a]");
  });
  test("star → bare", () => {
    expect(wrapSlot("*", { type: "star" }, false)).toBe("*");
  });
});

describe("buildCommandTex", () => {
  test("prefix: \\frac{a}{b}", () => {
    const record: TestRecord = {
      package: "base", name: "frac", type: "command",
      spec: "m m", kind: "prefix", allowed_mode: "math", tags: [],
    };
    expect(buildCommandTex(record, ["{a}", "{b}"])).toBe("\\frac{a}{b}");
  });

  test("infix: a \\above 1pt b", () => {
    const record: TestRecord = {
      package: "base", name: "above", type: "command",
      spec: "m:L", kind: "infix", allowed_mode: "math", tags: [],
    };
    expect(buildCommandTex(record, ["{1pt}"], ["1pt"])).toBe("a \\above 1pt b");
  });

  test("declarative: {\\bfseries a}", () => {
    const record: TestRecord = {
      package: "base", name: "bfseries", type: "command",
      spec: "", kind: "declarative", allowed_mode: "math", tags: [],
    };
    expect(buildCommandTex(record, [])).toBe("{\\bfseries a}");
  });
});

describe("buildEnvironmentTex", () => {
  test("matrix environment has correct body", () => {
    const record: TestRecord = {
      package: "ams", name: "Bmatrix", type: "environment",
      spec: "", allowed_mode: "math", tags: ["matrix", "nestable"],
    };
    const tex = buildEnvironmentTex(record, []);
    expect(tex).toContain("\\begin{Bmatrix}");
    expect(tex).toContain("a & b");
    expect(tex).toContain("\\end{Bmatrix}");
  });
});
