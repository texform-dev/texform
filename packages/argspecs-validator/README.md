# argspecs-validator

Validates TeXForm argument specs against live TeX renderers. Reads `resources/specs/*.yaml`, generates representative test cases for each command and environment, runs them through MathJax, KaTeX, and XeTeX, and writes structured results to `data/spec-tests/`.

## Prerequisites

**WASM build** — the ArgSpec parser is loaded from the local WASM package:

```bash
wasm-pack build crates/texform-wasm --target nodejs
```

**XeTeX** — required only when using `--xetex`.

## Usage

```bash
# Run all packages (MathJax + KaTeX + XeTeX)
bun run src/cli.ts

# Preview generated cases without running renderers
bun run src/cli.ts --dry-run

# Filter to a specific package or command
bun run src/cli.ts --package base
bun run src/cli.ts --package textmacros --name textbf

# Use a single renderer
bun run src/cli.ts --renderer mathjax

# Test a single record from JSON (outputs to stdout)
bun run src/cli.ts --record '{"package":"base","name":"frac","type":"command","argspec":"m m","kind":"prefix","allowed_mode":"math","tags":[]}'

# Force re-run, ignoring cached results
bun run src/cli.ts --force

# Custom output directory
bun run src/cli.ts --out-dir /tmp/spec-test-out
```

All three renderers run by default. If `xelatex` is not found in PATH, XeTeX is skipped automatically with a warning.

XeTeX tuning options:

| Flag | Default | Description |
|------|---------|-------------|
| `--xetex-batch-size` | `5` | Cases per XeTeX subprocess |
| `--xetex-concurrency` | `16` | Parallel XeTeX workers |

### Result Caching

On subsequent runs, cases whose results already exist in the output directory are skipped. A case is considered cached when all active renderers have a result for the same `(package, name, tex)` tuple. Use `--force` to bypass the cache and re-run everything.

If the generator produces different cases than what's cached (e.g. after a spec or generator change), new cases run automatically and stale cached entries are discarded.

## Test Case Generation

Each spec entry produces a set of cases via an **OFAT (One-Factor-At-a-Time)** strategy. Given an argspec like `o m:T`, the generator produces:

| Branch | Description | Example (`\cmd`) |
|--------|-------------|-----------------|
| `baseline` | Required slots filled; optional slots omitted | `\cmd{a}` |
| `vary:o[N]` | One optional slot enabled at a time | `\cmd[a]{b}` |
| `maximal` | All optional slots enabled (only if > 1) | `\cmd[a]{b}` |
| `bare[N]` | Required `standard`-form slot without `{}` wrapping | `\cmd a` |
| `neg:T[N]` | Negative: text-mode slot injected with `a^2` | `\cmd{a^2}` → **fail** |
| `neg:D[N]` | Negative: delimiter slot injected with `a` | → **fail** |
| `neg:L[N]` | Negative: dimension slot injected with `a` | → **fail** |
| `neg:I[N]` | Negative: integer slot injected with `a` | → **fail** |
| `neg:N[N]` | Negative: csname slot injected with `\alpha` | xetex **fail**, mathjax/katex pass |
| `nullable[N]` | `:D?` (nullable delimiter) slot tested with `{}` | → **pass** |

### Positive placeholders

Slots are filled with the simplest valid value for their type: letters (`a`, `b`, …) for content, `(` for delimiters, `1pt` for dimensions, `1` for integers, `k=v` for keyval, `cc` for column specs.

### Command assembly

The generated TeX differs by command `kind`:

- **prefix** (default): `\cmd{arg1}[arg2]…`
- **infix**: `a \cmd arg1 b`
- **declarative**: `{\cmd{arg1}… a}`

Environments wrap their body with `\begin{name}…\end{name}`. The body is `a` by default, `a & b \\\\ c & d` for matrix-tagged environments, and `a & b` for math-alignment environments.

## Custom Tests

Some commands need specific surrounding context that the generator cannot produce automatically (e.g. `\left` requires a matching `\right`). Override or supplement generated cases in `custom-tests/<package>.yaml`:

```yaml
commands:
  left:
    skip_generated: true   # discard all auto-generated cases
    cases:
      - branch: "ctx:paired"
        tex: "\\left( a \\right)"
        expect: pass
      - branch: "ctx:bracket"
        tex: "\\left[ a \\right]"
        expect: pass
```

- `skip_generated: true` replaces all auto-generated cases for that entry.
- Custom and generated cases are merged when `skip_generated` is absent or `false`.
- `expect` may be `"pass"`, `"fail"`, or a per-renderer object: `{ mathjax: "pass", katex: "fail", xetex: "fail" }`.

## Output

Results are written to `--out-dir` (default `out/spec-tests/`):

```
out/spec-tests/
├── results/<package>.jsonl   # RecordTestResult per line
├── errors/<package>.jsonl    # ErrorLogEntry per failing case
└── summary.json              # Aggregate counts by renderer and package
```

### `RecordTestResult` (one per spec entry)

```jsonc
{
  "package": "textmacros",
  "name": "textbf",
  "type": "command",
  "argspec": "m:T",
  "support": {
    "mathjax": "full",   // all positive cases passed
    "katex": "full",
    "xetex": "none"      // none passed (or renderer not run)
  },
  "cases": [
    {
      "branch": "baseline",
      "positive": true,
      "tex": "\\textbf{a}",
      "expect": "pass",
      "mathjax": true,
      "katex": true,
      "xetex": false,
      "errors": { "xetex": { "message": "…", "category": "unsupported" } }
    },
    {
      "branch": "neg:T[0]",
      "positive": false,
      "tex": "\\textbf{a^2}",
      "expect": "fail",
      "mathjax": false,
      "katex": false,
      "xetex": false
    }
  ]
}
```

`support` is derived from **positive** cases only (`full` / `partial` / `none`).

### Error categories

| Category | Meaning |
|----------|---------|
| `unsupported` | Renderer does not recognize the command or environment |
| `syntax_divergence` | Renderer accepts different syntax (e.g. bare token fails where `{}` passes) |
| `semantic_error` | Any other render-time error |
