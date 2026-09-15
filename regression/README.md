# Regression Data and Commands

Corpus inputs and tracked summaries live here; the binaries are implemented in `crates/texform-regression`. [TESTING.md](../TESTING.md) defines when checks are required. Run the commands below from the repository root.

## Data Preparation

`datasets.yaml` maps dataset slugs to Git LFS Parquet files under `data/`. Each record contains `formula_id` and `formula`; IDs use the first 12 hex characters of the normalized formula SHA-256, while deduplication uses the full hash.

Materialize LFS files before running checks:

```bash
git lfs install
git lfs pull
```

Inspect the selected and processed datasets in the output. Missing files or unresolved LFS pointers can be skipped; a partial run does not satisfy a full-corpus check.

| Dataset | Provenance |
| --- | --- |
| `unimer` | [UniMER](https://huggingface.co/datasets/wanderkid/UniMER_Dataset) |
| `wikipedia` | Formulas from `enwiki-20250820-pages-articles-multistream` |
| `linxy` | [LaTeX OCR](https://huggingface.co/datasets/linxy/LaTeX_OCR) |
| `lf80m-benchmarks` | Benchmark configs from [latex-formulas-80M](https://huggingface.co/datasets/OleehyO/latex-formulas-80M) |

## Parser Regression

For significant parser changes, run before and after and compare error rates. Real corpora contain invalid input, so absolute parse failures are expected; investigate increased failure rates and changed diagnostics.

```bash
cargo run --release -p texform-regression --bin parser_regression -- run --dry-run
```

`run --dry-run` prints results without writing files. Plain `run` writes summaries and commit details; neither is a substitute for checking results against the baseline. Use `verify` for an automated comparison with tracked summaries:

```bash
cargo run --release -p texform-regression --bin parser_regression -- verify --dataset lf80m-benchmarks
```

Verification requires matching stored results, so an intentional improvement can also require a baseline refresh. Investigate differences first, then refresh and review the tracked summary diff:

```bash
cargo run --release -p texform-regression --bin parser_regression -- refresh
```

`run` selects all configured datasets by default; use `--dataset` to narrow it. `verify` requires at least one `--dataset`; repeat the option to check multiple datasets. The pre-commit hook uses `refresh --probe-dataset lf80m-benchmarks`, refreshing all datasets only if the probe changes or is missing. CI verifies the probe dataset; neither replaces the required before/after review for significant parser changes.

To generate failure details without overwriting tracked results:

```bash
cargo run --release -p texform-regression --bin parser_regression -- run --dataset lf80m-benchmarks --emit-errors --skip-commit-results --results-root .tmp/parser-diagnostics
```

This writes a flat `errors.jsonl` and summaries under the supplied ignored `.tmp/` result root. Records include dataset, formula ID, source, strict/nonstrict mode, and diagnostics.

## Transform Contract

The checker runs the Corpus profile and verifies declared eliminated forms after the full pipeline. Transform execution errors and unlisted contract violations cause failure. It does not validate rendering fidelity or replace tests for other profiles.

A development probe:

```bash
cargo run --release -p texform-regression --bin transform_contract -- --dataset lf80m-benchmarks --dry-run
```

The full check required before merging transform changes:

```bash
cargo run --release -p texform-regression --bin transform_contract -- --dry-run
```

`--dry-run` writes no summary or detail files. If it fails, rerun the affected dataset without `--dry-run`, using a separate result root to preserve tracked summaries:

```bash
cargo run --release -p texform-regression --bin transform_contract -- --dataset lf80m-benchmarks --results-root .tmp/transform-diagnostics
```

Inspect that run's `commits/<hash>[-dirty]/violations.jsonl` and `errors.jsonl`; do not use stale files from an earlier run. Repeated diagnostic runs at the same commit reuse those paths. `--limit` is useful for probes but does not satisfy the full check.

`contract_exceptions.yaml` matches dataset, formula ID, 1-based occurrence within the same formula/target/node tuple, target kind/name, and optional node name. Each exception needs an English reason. Triage the actual violation before changing this allow-list; do not add broad exceptions to make a check pass.

## Counter Maps

`counter_dump` produces per-formula target counters for downstream analysis; it is not a correctness gate.

```bash
cargo run --release -p texform-regression --bin counter_dump
```

Each dataset is divided into chunks processed in fresh child processes to limit allocator retention. Small datasets produce `results/counter_map/<slug>.parquet`; larger ones produce `results/counter_map/<slug>/part-<offset>-<limit>.parquet`. Consumers can read either layout as a Parquet dataset without merging shards.

## Result Locations

Paths below are relative to this directory and describe default result roots. `--results-root` redirects a command's summaries and details.

| Path | Contents |
| --- | --- |
| `results/parser_regression/summary.json` | Tracked parser summary, with per-dataset entries |
| `results/parser_regression/commits/<hash>[-dirty]/<slug>/` | Ignored per-dataset summaries and `errors.jsonl` |
| `results/transform_contract/summary.json` | Tracked counts, exceptions, and verdict |
| `results/transform_contract/commits/<hash>[-dirty]/` | Ignored `violations.jsonl` and `errors.jsonl` |
| `results/counter_map/` | Counter-map data products |

The commit hash belongs to this texform repository even when invoked elsewhere with `--manifest-path`. Tracked summaries omit volatile timings and formula-level details. Plain transform runs write the tracked summary; use a separate result root for diagnosis and review any intentional summary refresh.
