# TeXForm Benchmarks

End-to-end benchmarks for the TeXForm parser. Each script runs `pytexform`
against local LaTeX corpora to capture throughput, percentile latencies, and
failure rates.

Currently shipped:

- **`extract_formulas.py`** — helper that pulls a single text column out of
  any directory of Parquet shards (e.g. a Hugging Face dataset snapshot with
  heavy image payloads) and writes a compact text-only Parquet to
  `benchmarks/cache/`.
- **`parse_datasets.py`** — parse success rate and timing against the
  Parquet sources declared in [`datasets.yaml`](./datasets.yaml).

## Prerequisites

From the `lib/texform/` repository root:

```bash
uv sync                # installs dev deps (polars, pyarrow, tqdm, ...)
uv run maturin develop # builds and installs pytexform into the venv
```

After that, `uv run python benchmarks/parse_datasets.py` uses the just-built
`pytexform` extension.

## Preparing a corpus

The upstream `OleehyO/latex-formulas-80M` dataset is ~640 GB on disk because
it bundles rendered images alongside the formula text. `extract_formulas.py`
reads only the requested column (column pruning avoids materializing images)
and produces a compact text-only Parquet.

The dataset contains several immediate subdirectories (*subsets*):

| Subset | Description |
|---|---|
| `benchmark_complex` | Complex multi-operator expressions |
| `benchmark_matrix` | Matrix / array environments |
| `benchmark_ordinary` | Common single-line formulas |
| `benchmark_sample` | Mixed random sample |
| `benchmark_symbol` | Symbol-heavy formulas |
| `benchmark_text_hybrid` | Text and math mixed |
| `en` | Large English corpus (~78 M rows, images included) |
| `zh` | Large Chinese corpus |

For the parse benchmark we use only the four `benchmark_*` subsets listed
above (images-free, manageably sized):

```bash
# Recommended: extract only the four benchmark subsets
uv run python benchmarks/extract_formulas.py \
    --input /data_disk/latex-formula-datasets/OleehyO__latex-formulas-80M \
    --subset benchmark_complex benchmark_matrix benchmark_ordinary benchmark_sample \
    --name latex-formulas-80m-benchmark

# Full corpus (78 M rows, ~2.9 GB output — skips nothing)
uv run python benchmarks/extract_formulas.py \
    --input /data_disk/latex-formula-datasets/OleehyO__latex-formulas-80M \
    --name latex-formulas-80m

# Custom column / output name with deduplication
uv run python benchmarks/extract_formulas.py \
    --input /path/to/shards \
    --column latex_formula \
    --name my-corpus \
    --dedupe
```

The default output path is `benchmarks/cache/<input-basename>.parquet`. Update
`datasets.yaml` to point at the generated file (see **Adding a new corpus**).

## Running the parse benchmark

```bash
# Full run using every source in datasets.yaml
uv run python benchmarks/parse_datasets.py

# Smoke test -- cap each source to the first N rows while loading
uv run python benchmarks/parse_datasets.py --limit 10000

# Filter to a single configured source
uv run python benchmarks/parse_datasets.py --sources latex-formulas-80m

# Override worker count and output directory
uv run python benchmarks/parse_datasets.py --workers 8 --output-dir /tmp/pt-bench
```

Every invocation wipes its output directory and starts fresh -- typical runs
finish in a minute or two, so there is no resume/checkpoint machinery.

## Output layout

By default, outputs land in:

```
benchmarks/results/parse_datasets/
├── summary.json              # rolling snapshot, tracked in git
└── commit-<hash8>/           # one directory per git commit (ignored)
    ├── manifest.json         # run config and preprocessing stats
    ├── results.parquet       # one row per unique formula
    ├── errors.jsonl          # strict-mode failures + full diagnostics
    └── summary.json          # identical copy of the rolling snapshot above
```

`<hash8>` is the first 8 characters of `git rev-parse HEAD`. The commit
directory is `.gitignored`; only the rolling `summary.json` at the top level
is committed. Passing `--output-dir PATH` overrides the default and skips the
rolling copy when `PATH` is outside `benchmarks/results/parse_datasets/`.

## Tracking performance over time

The rolling `summary.json` is the only artifact we commit. Any change to
`texform-core` or the Python binding shows up as a diff on that file after
the benchmark is re-run, so performance regressions surface during code
review.

Summary schema (abbreviated):

```json
{
  "commit_hash": "54fbcf0a",
  "sources": [{ "name": "latex-formulas-80m", "files": [...], ... }],
  "preprocessing_stats": { "raw_counts": {...}, "unique_formula_count": ... },
  "completed": 1200000,
  "strict":    { "ok": ..., "failed": ..., "failure_rate_pct": 22.08,
                 "timing_ms": { "p50": 0.4, "p95": 2.3, "p99": 4.9 } },
  "nonstrict": { ... }
}
```

## Adding a new corpus

1. Run `extract_formulas.py` on the upstream shards (or point directly at a
   directory you own if it already contains text-only Parquet).
2. Add an entry to [`datasets.yaml`](./datasets.yaml):

   ```yaml
   sources:
     - name: my-corpus
       files:
         - cache/my-corpus.parquet
         # glob patterns are fine too:
         # - /data/extra/*.parquet
       column: latex_formula
       # max_rows: 5_000_000   # optional per-source cap
   ```

Each source contributes `<sanitized_name>_count` and
`<sanitized_name>_sample_id` columns to `results.parquet`, so downstream
analysis can attribute every unique formula back to its origin.

## How the parse pipeline works

1. For each source, scan the configured Parquet files and project the
   requested column only (column pruning).
2. Normalize the formula (newline canonicalization, strip a single outer
   `$...$` / `\[...\]` / `\(...\)` pair when it actually wraps the whole
   string, collapse whitespace).
3. Drop formulas shorter than 5 non-whitespace characters.
4. Globally deduplicate by `sha256(normalized)`, keeping per-source
   occurrence counts and a representative sample id.
5. Parse every unique formula twice (`strict=True` and `strict=False`) in a
   `ProcessPoolExecutor`, buffering rows in memory.
6. Write `results.parquet`, compute aggregates, and emit `summary.json`
   (plus the rolling copy at the top of `results/parse_datasets/`).
