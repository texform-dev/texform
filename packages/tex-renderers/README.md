# @texform/tex-renderers

Unified TeX compilation adapters for MathJax, KaTeX, and XeTeX. Used by
`argspecs-validator` to test whether LaTeX commands compile successfully across
renderers.

## Usage

```ts
import {
  createMathJaxCompiler,
  createKaTeXCompiler,
  createXeTeXCompiler,
  createXeTeXBatchCompiler,
} from "@texform/tex-renderers";

const compiler = createXeTeXCompiler();
const result = await compiler.compile("\\frac{a}{b}", {
  packages: ["base"],
  mode: "math",
});
// { success: true }
```

All adapters share a common interface:

```ts
interface TexCompiler {
  name: "mathjax" | "katex" | "xetex";
  compile(tex: string, options: CompileOptions): Promise<CompileResult>;
  dispose?(): Promise<void>;
}

interface CompileOptions {
  packages: string[];        // Logical package names: "base", "ams", "physics", …
  mode: "math" | "text";    // Math wraps in $…$; text is passed as-is
  display?: boolean;         // Display mode (MathJax/KaTeX only)
}

interface CompileResult {
  success: boolean;
  error?: string;
  output?: string;           // Rendered output (MathJax/KaTeX only)
}
```

## Adapters

### MathJax

In-process compilation via `mathjax` npm package. Converts TeX → MathML and
checks for `<merror>` nodes.

### KaTeX

In-process compilation via `katex` npm package with `throwOnError: true`.

### XeTeX

Spawns `xelatex` with `-interaction=nonstopmode -halt-on-error -no-pdf`.
Determines success by exit code plus log inspection — commands that produce
`LaTeX Warning: Command \X invalid` (e.g. text accents used in math mode) are
treated as failures even when the exit code is 0.

Requires `xelatex` on `PATH`.

### XeTeX Batch

Groups multiple items by compile options, compiles each group in a single
`xelatex` invocation using `\typeout` markers for per-case attribution, and
falls back to individual compilation on error to pinpoint which case failed.

```ts
const batch = createXeTeXBatchCompiler({
  batchSize: 5,      // Items per xelatex invocation (default: 5)
  concurrency: 16,   // Parallel workers (default: 16)
});
const results = await batch.compileBatch(items);
await batch.dispose();
```

## Package Map

Logical package names are mapped to renderer-specific packages via
`PACKAGE_MAP`:

| Logical     | XeTeX `\usepackage` | MathJax extension | KaTeX |
|-------------|---------------------|-------------------|-------|
| `base`      | _(none)_            | `base`            | yes   |
| `ams`       | `amsmath`           | `ams`             | yes   |
| `physics`   | `physics`           | `physics`         | no    |
| `boldsymbol`| `bm`                | `boldsymbol`      | yes   |
| `textmacros`| _(none)_            | `textmacros`      | yes   |
| `bboldx`    | `bboldx`            | `bboldx`          | no    |

## Testing

```bash
bun test
```
