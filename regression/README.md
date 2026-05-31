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

# dump per-dataset counter map shards for workspace analysis
# each dataset is sliced into fixed-size chunks; every chunk runs in a fresh
# `--direct` child process so allocator retention cannot accumulate across the run
cargo run --release -p texform-regression --bin counter_dump
```

## Results

- `results/parser_regression/summary.json` — tracked current parser regression summary; per-dataset entries live under `datasets`
- `results/parser_regression/commits/<hash>/<slug>/summary.json` — ignored per-dataset snapshot for that commit
- `results/parser_regression/commits/<hash>/<slug>/errors.jsonl` — ignored strict and nonstrict failures with full diagnostics
- `results/counter_map/` — per-formula target counter rows for workspace analysis. The layout switches by dataset size:
  - `results/counter_map/<slug>.parquet` when the dataset fits in a single chunk (the conventional small-dataset layout);
  - `results/counter_map/<slug>/part-<offset>-<limit>.parquet` when the dataset spans multiple chunks. Downstream consumers (Polars / PyArrow / DuckDB) read either form as a parquet dataset, so no merge step is needed.

`<hash>` is the HEAD of this texform repository, even when the command is invoked from an outer workspace with
`--manifest-path`.
