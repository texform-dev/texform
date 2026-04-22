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

  test("isolates package-specific commands across compiles", async () => {
    const physics = await compiler.compile("\\ketbra{a}", {
      packages: ["physics"],
      mode: "math",
    });
    expect(physics.success).toBe(true);

    const braket = await compiler.compile("\\ketbra{a}", {
      packages: ["braket"],
      mode: "math",
    });
    expect(braket.success).toBe(false);
    expect(braket.error).toContain("Missing argument");

    const physicsAgain = await compiler.compile("\\ketbra{a}", {
      packages: ["physics"],
      mode: "math",
    });
    expect(physicsAgain.success).toBe(true);
  });

  test("does not leak state across compiles in the same package", async () => {
    const first = await compiler.compile("\\label{a}", {
      packages: ["base"],
      mode: "math",
    });
    expect(first.success).toBe(true);

    const second = await compiler.compile("\\label a", {
      packages: ["base"],
      mode: "math",
    });
    expect(second.success).toBe(true);
  });

  test("returns a compile error for unknown packages", async () => {
    const result = await compiler.compile("\\frac{a}{b}", {
      packages: ["does-not-exist"],
      mode: "math",
    });

    expect(result.success).toBe(false);
    expect(result.error).toContain("Unknown TeX package");
  });
});
