import { describe, test, expect } from "bun:test";
import { createXeTeXCompiler } from "../src/adapters/xetex.js";
import { createXeTeXBatchCompiler } from "../src/adapters/xetex-batch.js";

describe("XeTeX adapter", () => {
  const compiler = createXeTeXCompiler();

  test("valid math compiles successfully", async () => {
    const result = await compiler.compile("\\frac{a}{b}", {
      packages: ["base"],
      mode: "math",
    });
    expect(result.success).toBe(true);
  });

  test("invalid TeX produces error", async () => {
    const result = await compiler.compile("\\frac{a}", {
      packages: ["base"],
      mode: "math",
    });
    expect(result.success).toBe(false);
  });

  test("text mode does not add math wrapping", async () => {
    const result = await compiler.compile("\\textbf{hello}", {
      packages: ["base"],
      mode: "text",
    });
    expect(result.success).toBe(true);
  });

  describe("invalid-command warning detection", () => {
    const textAccents = ['\\"{a}', "\\'{a}", "\\.{a}", "\\={a}", "\\^{a}", "\\`{a}", "\\~{a}"];

    test("text accents in math mode are rejected", async () => {
      for (const tex of textAccents) {
        const result = await compiler.compile(tex, { packages: ["base"], mode: "math" });
        expect(result.success).toBe(false);
        expect(result.error).toMatch(/^LaTeX Warning: Command .+ invalid/);
      }
    });

    test("text accents in text mode still pass", async () => {
      for (const tex of textAccents) {
        const result = await compiler.compile(tex, { packages: ["textmacros"], mode: "text" });
        expect(result.success).toBe(true);
      }
    });

    test("math accents in math mode are not affected", async () => {
      for (const tex of ["\\hat{a}", "\\acute{a}", "\\dot{a}", "\\vec{a}"]) {
        const result = await compiler.compile(tex, { packages: ["base"], mode: "math" });
        expect(result.success).toBe(true);
      }
    });
  });
});

describe("XeTeX batch adapter", () => {
  const batch = createXeTeXBatchCompiler({ batchSize: 5, concurrency: 1 });

  test("warnings are attributed to the correct case in a batch", async () => {
    const items = [
      { tex: "\\frac{a}{b}", options: { packages: ["base"], mode: "math" as const } },
      { tex: '\\"{a}',       options: { packages: ["base"], mode: "math" as const } },
      { tex: "\\hat{a}",     options: { packages: ["base"], mode: "math" as const } },
      { tex: "\\'{a}",       options: { packages: ["base"], mode: "math" as const } },
      { tex: "\\sqrt{x}",    options: { packages: ["base"], mode: "math" as const } },
    ];

    const results = await batch.compileBatch(items);

    expect(results[0].success).toBe(true);
    expect(results[1].success).toBe(false);
    expect(results[2].success).toBe(true);
    expect(results[3].success).toBe(false);
    expect(results[4].success).toBe(true);

    expect(results[1].error).toMatch(/Command \\" invalid/);
    expect(results[3].error).toMatch(/Command \\' invalid/);
  });
});
