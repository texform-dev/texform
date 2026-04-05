import { describe, test, expect } from "bun:test";
import { createMathJaxCompiler } from "../src/adapters/mathjax.js";

describe("MathJax adapter", () => {
  const compiler = createMathJaxCompiler();

  test("valid math compiles successfully", async () => {
    const result = await compiler.compile("\\frac{a}{b}", {
      packages: ["base"],
      mode: "math",
    });
    expect(result.success).toBe(true);
    expect(result.output).toContain("<mfrac");
  });

  test("missing argument produces error", async () => {
    const result = await compiler.compile("\\frac{a}", {
      packages: ["base"],
      mode: "math",
    });
    expect(result.success).toBe(false);
    expect(result.error).toContain("Missing argument");
  });

  test("text mode wraps with \\text{}", async () => {
    const result = await compiler.compile("\\textbf{hello}", {
      packages: ["base", "textmacros"],
      mode: "text",
    });
    expect(result.success).toBe(true);
  });

  test("math-in-text produces error", async () => {
    const result = await compiler.compile("\\textbf{a^2}", {
      packages: ["base", "textmacros"],
      mode: "text",
    });
    expect(result.success).toBe(false);
  });
});
