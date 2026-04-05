import { describe, test, expect } from "bun:test";
import { loadCustomTests, type CustomTestConfig } from "../src/custom-tests.js";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

describe("loadCustomTests", () => {
  function setupDir(files: Record<string, string>): string {
    const dir = mkdtempSync(join(tmpdir(), "ct-"));
    for (const [name, content] of Object.entries(files)) {
      writeFileSync(join(dir, name), content);
    }
    return dir;
  }

  test("loads commands and environments from YAML", () => {
    const dir = setupDir({
      "base.yaml": `
commands:
  left:
    skip_ofat: true
    cases:
      - branch: "ctx:basic"
        tex: "\\\\left( a \\\\right)"
        expect: pass
environments:
  split:
    skip_ofat: true
    cases:
      - branch: "ctx:in-eq"
        tex: "\\\\begin{equation}\\\\begin{split} a \\\\end{split}\\\\end{equation}"
        expect: pass
`,
    });
    const map = loadCustomTests(dir);
    expect(map.get("base/command/left")).toBeDefined();
    expect(map.get("base/command/left")!.skip_ofat).toBe(true);
    expect(map.get("base/command/left")!.cases).toHaveLength(1);
    expect(map.get("base/environment/split")).toBeDefined();
  });

  test("returns empty map for missing directory", () => {
    const map = loadCustomTests("/nonexistent/path");
    expect(map.size).toBe(0);
  });

  test("defaults skip_ofat to false", () => {
    const dir = setupDir({
      "ams.yaml": `
commands:
  tag:
    cases:
      - branch: "ctx:basic"
        tex: "\\\\begin{equation}\\\\tag{1} a\\\\end{equation}"
        expect: pass
`,
    });
    const map = loadCustomTests(dir);
    expect(map.get("ams/command/tag")!.skip_ofat).toBe(false);
  });

  test("handles per-renderer expect", () => {
    const dir = setupDir({
      "base.yaml": `
commands:
  left:
    skip_ofat: true
    cases:
      - branch: "ctx:mismatch"
        tex: "\\\\left( a \\\\right]"
        expect:
          mathjax: pass
          katex: pass
          xetex: fail
`,
    });
    const map = loadCustomTests(dir);
    const c = map.get("base/command/left")!.cases[0];
    expect(c.expect).toEqual({ mathjax: "pass", katex: "pass", xetex: "fail" });
  });
});
