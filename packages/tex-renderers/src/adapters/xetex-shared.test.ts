import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import {
  buildSingleTexFile,
  buildUsepackages,
  extractXeTeXError,
  findInvalidCommandWarning,
} from "./xetex-shared.js";

describe("XeTeX shared helpers", () => {
  test("buildUsepackages maps enabled renderer packages to XeTeX packages", () => {
    expect(buildUsepackages(["base", "ams", "boldsymbol", "unknown"])).toBe(
      "\\usepackage{amsmath}\n\\usepackage{bm}",
    );
  });

  test("buildSingleTexFile wraps math input and inserts usepackage directives", () => {
    expect(
      buildSingleTexFile("\\alpha", {
        packages: ["ams", "bboldx"],
        mode: "math",
      }),
    ).toBe(`\\documentclass{article}
\\usepackage{amsmath}
\\usepackage{bboldx}
\\begin{document}
$\\alpha$
\\end{document}
`);
  });

  test("findInvalidCommandWarning returns the first invalid command warning", () => {
    const log = [
      "Package info",
      "LaTeX Warning: Command \\foo invalid in math mode on input line 7.",
      "LaTeX Warning: Command \\bar invalid in math mode on input line 8.",
    ].join("\n");

    expect(findInvalidCommandWarning(log)).toBe(
      "LaTeX Warning: Command \\foo invalid in math mode on input line 7.",
    );
  });

  test("extractXeTeXError prefers stderr fatal errors over the log", async () => {
    const dir = await mkdtemp(join(tmpdir(), "xetex-shared-test-"));
    const logPath = join(dir, "input.log");

    try {
      await writeFile(logPath, "! Log error\nmore details");
      expect(await extractXeTeXError(logPath, "! Stderr error\nother")).toBe(
        "! Stderr error",
      );
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  test("extractXeTeXError falls back to fatal errors in the log", async () => {
    const dir = await mkdtemp(join(tmpdir(), "xetex-shared-test-"));
    const logPath = join(dir, "input.log");

    try {
      await writeFile(logPath, "noise\n! Log error\nmore details");
      expect(await extractXeTeXError(logPath, "")).toBe("! Log error");
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  test("extractXeTeXError can preserve batch fallback log-first behavior", async () => {
    const dir = await mkdtemp(join(tmpdir(), "xetex-shared-test-"));
    const logPath = join(dir, "input.log");

    try {
      await writeFile(logPath, "! Log error\nmore details");
      expect(
        await extractXeTeXError(logPath, "! Stderr error\nother", "log-first"),
      ).toBe("! Log error");
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
