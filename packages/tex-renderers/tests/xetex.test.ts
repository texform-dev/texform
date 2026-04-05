import { describe, test, expect } from "bun:test";
import { createXeTeXCompiler } from "../src/adapters/xetex.js";

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
});
