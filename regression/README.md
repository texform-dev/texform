# Regression Data

This directory contains corpus inputs and tracked regression outputs for `texform-regression`.

## Layout

- `data/` — tracked Parquet datasets used by corpus regression
- `datasets.yaml` — slug-to-file mapping consumed by `texform-regression`
- `results/` — current summaries, ignored commit snapshots, counter-map working files

Each dataset parquet stores `formula_id` and `formula`.
`formula_id` is the first 12 hex chars of the normalized formula SHA-256, while dedup still uses the full hash.

Before running corpus regression, materialize the dataset files with Git LFS:

```bash
# from texform repo root
git lfs install && git lfs pull
```

## Dataset Provenance

- `unimer`
  Source: https://huggingface.co/datasets/wanderkid/UniMER_Dataset

- `wikipedia`
  Source: formulas extracted from `enwiki-20250820-pages-articles-multistream`

- `linxy`
  Source: https://huggingface.co/datasets/linxy/LaTeX_OCR

- `lf80m-benchmarks`
  Source: benchmark configs from https://huggingface.co/datasets/OleehyO/latex-formulas-80M

## Run

```bash
# from texform repo root

# run all datasets
cargo run --release -p texform-regression --bin parser_regression

# run one dataset
cargo run --release -p texform-regression --bin parser_regression -- --dataset lf80m-benchmarks

# pre-commit probe: check one dataset first, then refresh all results if it changed or is missing
cargo run --release -p texform-regression --bin parser_regression -- --dataset lf80m-benchmarks --check

# run the transform eliminated-form contract gate
cargo run --release -p texform-regression --bin transform_contract

# run a small transform contract probe
cargo run --release -p texform-regression --bin transform_contract -- --dataset lf80m-benchmarks --limit 1000

# dump per-dataset counter map shards for downstream analysis
# each dataset is sliced into fixed-size chunks; every chunk runs in a fresh
# `--direct` child process so allocator retention cannot accumulate across the run
cargo run --release -p texform-regression --bin counter_dump
```

## Results

- `results/parser_regression/summary.json` — tracked current parser regression summary; per-dataset entries live under `datasets`
- `results/parser_regression/commits/<hash>/<slug>/summary.json` — ignored per-dataset snapshot for that commit
- `results/parser_regression/commits/<hash>/<slug>/errors.jsonl` — ignored strict and nonstrict failures with full diagnostics
- `results/transform_contract/summary.json` — tracked current transform eliminated-form contract summary
- `results/transform_contract/commits/<hash>[-dirty]/violations.jsonl` — ignored formula-level transform contract violations
- `results/transform_contract/commits/<hash>[-dirty]/errors.jsonl` — ignored parse and non-contract transform errors
- `results/counter_map/` — per-formula target counter rows for downstream analysis. The layout switches by dataset size:
  - `results/counter_map/<slug>.parquet` when the dataset fits in a single chunk (the conventional small-dataset layout);
  - `results/counter_map/<slug>/part-<offset>-<limit>.parquet` when the dataset spans multiple chunks. Downstream consumers (Polars / PyArrow / DuckDB) read either form as a parquet dataset, so no merge step is needed.

`<hash>` is the HEAD of this texform repository, even when the command is invoked from another directory with
`--manifest-path`.

`summary.json` is intentionally stable enough to track in git; run timings, formula-level detail, and
rule attribution stay out of this file. The transform contract summary contains `schema_version`,
`metadata`, `checked_formulas`, parser/transform/contract error counts, `violating_formulas`,
`violations`, exception counts, `unexpected_violations`, and a `verdict`.
Formula-level detail stays in the ignored `commits/<hash>[-dirty]/` directory.

`contract_exceptions.yaml` is the transform contract allow-list. Exceptions are matched by dataset,
formula id, 1-based occurrence within the same formula/target/node tuple, target kind, target name,
and optional node name; each entry must include an English reason. Do not add broad exceptions for
new violations without first triaging the generated detail files.
