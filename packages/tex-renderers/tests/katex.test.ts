import { describe, test, expect } from "bun:test";
import { createKaTeXCompiler } from "../src/adapters/katex.js";

describe("KaTeX adapter", () => {
  const compiler = createKaTeXCompiler();

  test("valid math compiles successfully", async () => {
    const result = await compiler.compile("\\frac{a}{b}", {
      packages: ["base"],
      mode: "math",
    });
    expect(result.success).toBe(true);
  });

  test("missing argument produces error", async () => {
    const result = await compiler.compile("\\frac{a}", {
      packages: ["base"],
      mode: "math",
    });
    expect(result.success).toBe(false);
    expect(result.error?.toLowerCase()).toContain("expected");
  });

  test("unknown command produces error", async () => {
    const result = await compiler.compile("\\nonexistentcommand", {
      packages: ["base"],
      mode: "math",
    });
    expect(result.success).toBe(false);
  });
});
