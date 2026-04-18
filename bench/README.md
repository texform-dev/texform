# Bench Data

This directory contains the corpus bench inputs and outputs for `texform-bench`.

## Layout

- `data/` — tracked Parquet datasets used by the bench
- `datasets.yaml` — slug-to-file mapping consumed by `texform-bench`
- `results/` — `overall.json`, per-dataset summaries, and commit snapshots

Each dataset parquet stores `formula_id` and `formula`.
`formula_id` is the first 12 hex chars of the normalized formula SHA-256, while dedup still uses the full hash.

Before running the bench, materialize the dataset files with Git LFS:

```bash
cd lib/texform
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
cd lib/texform

# run all datasets
cargo bench -p texform-bench --bench parse_corpus

# run one dataset
cargo bench -p texform-bench --bench parse_corpus -- --dataset lf80m-benchmarks
```

## Results

- `results/overall.json` — aggregated counts, failure rates, and timing percentiles across the current bench run
- `results/<slug>/summary.json` — per-dataset counts, failure rates, timing percentiles, and `timing_ms.max_formula_id`
- `results/commits/<hash>/<slug>/summary.json` — per-dataset snapshot for that commit
- `results/commits/<hash>/<slug>/errors.jsonl` — strict and nonstrict failures with full diagnostics
