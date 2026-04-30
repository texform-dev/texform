import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { PACKAGE_MAP } from "../package-map.js";
import type { CompileOptions, CompileResult } from "../types.js";

export const INVALID_CMD_RE = /^LaTeX Warning: Command .+ invalid/;

export interface XeTeXCompileItem {
  tex: string;
  options: CompileOptions;
}

export type XeTeXErrorPreference = "stderr-first" | "log-first";

export function buildUsepackages(packages: string[]): string {
  return packages
    .flatMap((p) => PACKAGE_MAP[p]?.xetex ?? [])
    .map((pkg) => `\\usepackage{${pkg}}`)
    .join("\n");
}

export function buildSingleTexFile(tex: string, options: CompileOptions): string {
  const usepackages = buildUsepackages(options.packages);
  const body = options.mode === "math" ? `$${tex}$` : tex;

  return `\\documentclass{article}
${usepackages}
\\begin{document}
${body}
\\end{document}
`;
}

export function spawnXeLaTeX(inputPath: string, outDir: string) {
  return Bun.spawn(
    [
      "xelatex",
      "-interaction=nonstopmode",
      "-halt-on-error",
      "-no-pdf",
      `-output-directory=${outDir}`,
      inputPath,
    ],
    { stdout: "pipe", stderr: "pipe" },
  );
}

export async function extractXeTeXError(
  logPath: string,
  stderr: string,
  preference: XeTeXErrorPreference = "stderr-first",
): Promise<string> {
  if (preference === "log-first") {
    try {
      const log = await readFile(logPath, "utf8");
      const logLine = log.split("\n").find((line) => line.startsWith("!"));
      if (logLine) return logLine;
      return log.slice(0, 300);
    } catch {
      return stderr.slice(0, 200) || "unknown xelatex error";
    }
  }

  const stderrLine = stderr.split("\n").find((line) => line.startsWith("!"));
  if (stderrLine) return stderrLine;

  try {
    const log = await readFile(logPath, "utf8");
    const logLine = log.split("\n").find((line) => line.startsWith("!"));
    if (logLine) return logLine;
    return log.slice(0, 300);
  } catch {
    return stderr.slice(0, 200) || "unknown xelatex error";
  }
}

export function findInvalidCommandWarning(log: string): string | undefined {
  return log.split("\n").find((line) => INVALID_CMD_RE.test(line));
}

export async function compileSingleXeTeX(
  item: XeTeXCompileItem,
  tmpPrefix: string,
  errorPreference: XeTeXErrorPreference = "stderr-first",
): Promise<CompileResult> {
  const dir = await mkdtemp(join(tmpdir(), tmpPrefix));
  const texPath = join(dir, "input.tex");
  const logPath = join(dir, "input.log");

  try {
    await writeFile(texPath, buildSingleTexFile(item.tex, item.options));

    const proc = spawnXeLaTeX(texPath, dir);
    const exitCode = await proc.exited;
    const stderr = await new Response(proc.stderr).text();

    if (exitCode !== 0) {
      const error = await extractXeTeXError(logPath, stderr, errorPreference);
      return { success: false, error };
    }

    try {
      const log = await readFile(logPath, "utf8");
      const warning = findInvalidCommandWarning(log);
      if (warning) return { success: false, error: warning };
    } catch {}

    return { success: true };
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}
