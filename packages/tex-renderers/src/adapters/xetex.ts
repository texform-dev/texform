import { mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { PACKAGE_MAP } from "../package-map.js";
import type { CompileOptions, CompileResult, TexCompiler } from "../types.js";

const INVALID_CMD_RE = /^LaTeX Warning: Command .+ invalid/;

function buildTexFile(tex: string, options: CompileOptions): string {
  const usepackages = options.packages
    .flatMap((p) => PACKAGE_MAP[p]?.xetex ?? [])
    .map((pkg) => `\\usepackage{${pkg}}`)
    .join("\n");

  const body = options.mode === "math" ? `$${tex}$` : tex;

  return `\\documentclass{article}
${usepackages}
\\begin{document}
${body}
\\end{document}
`;
}

// xelatex writes errors to stdout (the .log file) rather than stderr.
// We try stderr first, then fall back to reading the .log file.
async function extractError(
  stderr: string,
  logPath: string,
): Promise<string> {
  const stderrLine = stderr.split("\n").find((l) => l.startsWith("!"));
  if (stderrLine) return stderrLine;

  try {
    const log = await readFile(logPath, "utf8");
    const logLine = log.split("\n").find((l) => l.startsWith("!"));
    if (logLine) return logLine;
    // Return the first meaningful portion of the log as a fallback.
    return log.slice(0, 300);
  } catch {
    return stderr.slice(0, 200) || "unknown xelatex error";
  }
}

export function createXeTeXCompiler(): TexCompiler {
  return {
    name: "xetex",
    async compile(tex, options) {
      const dir = await mkdtemp(join(tmpdir(), "xetex-"));
      const texPath = join(dir, "input.tex");
      const logPath = join(dir, "input.log");

      try {
        await writeFile(texPath, buildTexFile(tex, options));

        const proc = Bun.spawn(
          [
            "xelatex",
            "-interaction=nonstopmode",
            "-halt-on-error",
            "-no-pdf",
            `-output-directory=${dir}`,
            texPath,
          ],
          { stdout: "pipe", stderr: "pipe" },
        );
        const exitCode = await proc.exited;
        const stderr = await new Response(proc.stderr).text();

        if (exitCode !== 0) {
          const errorLine = await extractError(stderr, logPath);
          return { success: false, error: errorLine };
        }

        try {
          const log = await readFile(logPath, "utf8");
          const warning = log.split("\n").find((l) => INVALID_CMD_RE.test(l));
          if (warning) return { success: false, error: warning };
        } catch {}

        return { success: true };
      } finally {
        await rm(dir, { recursive: true, force: true });
      }
    },
  };
}
