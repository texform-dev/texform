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

`--dry-run` writes no summary or detail files. If it fails, rerun the affected dataset without `--dry-run` to write failure details, for example under a separate result root:

```bash
cargo run --release -p texform-regression --bin transform_contract -- --dataset lf80m-benchmarks --results-root .tmp/transform-diagnostics
```

Inspect that run's `commits/<hash>[-dirty]/violations.jsonl` and `errors.jsonl`; do not use stale files from an earlier run. Repeated diagnostic runs at the same commit reuse those paths. `--limit` is useful for probes but does not satisfy the full check.

`contract_exceptions.yaml` matches dataset, formula ID, 1-based occurrence within the same formula/target/node tuple, target kind/name, and optional node name. Each exception needs an English reason. Triage the actual violation before changing this allow-list; do not add broad exceptions to make a check pass.

## Round-Trip Report

`--round-trip-report` adds a measurement of the transformed-tree round trip (see [Structural Round Trips](../TESTING.md#structural-round-trips)). It runs the round-trip checks on every configured dataset and on a generated `adversarial-boundary` dataset, under three fixed package sets (`default`, `physics`, and `mixed`, the defaults plus `physics`), four profiles, and two output styles. The adversarial inputs are derived from the knowledge base and the rewrite plans and use the formula as their ID. The report is an observation, not a merge condition: the Corpus eliminated-form check runs as usual, and the exit status, verdict, and `summary.json` depend on that check alone.

The full report, which takes about ten minutes on a 32-core machine:

```bash
cargo run --release -p texform-regression --bin transform_contract -- --round-trip-report
```

A development probe, which prints counts and samples without writing files:

```bash
cargo run --release -p texform-regression --bin transform_contract -- --round-trip-report --dataset lf80m-benchmarks --limit 5000 --dry-run
```

A run writes the ignored `results/transform_contract/round_trip_report.json`. For each dataset, package set, profile, and style it records the checked inputs (`checked`), inputs skipped because the parse is incomplete or the transform failed (`skipped`), and failures per check (`failures`, keyed by the check names `reparse`, `reparse_error`, `retransform`, and `reparse_retransform`). The `parser_round_trip` check does not depend on the profile and is counted per dataset, package set, and style. Totals across datasets follow. Its provenance has a config hash over the inputs only: dataset selection, `--limit`, the package sets, and the adversarial generator version. Transform code and configuration are excluded, because the report measures their effect.

Every failure, including each transform failure, is written to the ignored `commits/<hash>[-dirty]/round_trip_failures.jsonl` with the dataset, formula ID, formula, package set, profile, style, check, output text, and, for `reparse`, the first differing node path and the node names on both sides. Aggregate it ad hoc, for example by the first differing node.

Each run first reads the report that the previous run left in the same result root. With the same config hash, it prints every changed cell as before → after with the difference, names the commit of the earlier report, and prints up to three samples for cells whose counts rose. With a different hash, such as after `--limit` or `--dataset`, or without an earlier report, it prints this run's counts only. To compare with another commit, run the report there first with `--results-root <directory>`, then run it on your change with the same `--results-root`. The adversarial inputs change with rules and knowledge records, so compare its checked counts and details as well as its failure counts.

Transform changes should run the full report and state the per-cell changes, including transform failures, in the change description. Rising counts need investigation, but the report itself never fails a check.

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
| `results/transform_contract/summary.json` | Ignored counts, exceptions, and verdict of the last run |
| `results/transform_contract/commits/<hash>[-dirty]/` | Ignored `violations.jsonl` and `errors.jsonl` |
| `results/transform_contract/round_trip_report.json` | Ignored round-trip counts of the last run per dataset, package set, profile, and style |
| `results/transform_contract/commits/<hash>[-dirty]/round_trip_failures.jsonl` | Ignored round-trip failure details |
| `results/counter_map/` | Counter-map data products |

The commit hash belongs to this texform repository even when invoked elsewhere with `--manifest-path`. The tracked parser summary omits volatile timings and formula-level details. Transform contract results are not tracked, because they can be recomputed from the datasets; state the counts that matter in the change description.

## Whole-Tree Conformance Audits

The optional `--check-conformance` flag checks parser output and every transform profile against the document invariants. It is disabled by default and does not change the stored regression summary schema or the ordinary parser workload. Parser checks run after both parse timers stop; overall audit wall time includes validation and must not be compared with ordinary regression wall time.

Run the complete audits without the parser's usual length filter:

```bash
cargo run --release -p texform-regression --bin parser_regression -- run --dry-run --check-conformance --max-formula-len 18446744073709551615
cargo run --release -p texform-regression --bin transform_contract -- --dry-run --check-conformance
```

The parser command above assumes a 64-bit host. Every complete strict and nonstrict result is checked. Missing documents and trees containing `Error` nodes are counted as incomplete and skipped. The transform audit runs Authoring, Faithful, Corpus, and Equiv separately on each complete parsed document; the existing Corpus eliminated-form check and its formula-specific exceptions remain in force. Conformance failures cannot be allowed by those exceptions.

Each dataset reports the number of checked and incomplete trees for each mode or profile, along with violation and transform-error counts. Every conformance failure emits a JSON record to stderr containing the dataset, formula ID, source formula, mode or profile, node path, rule code, and message, including with `--dry-run`. Redirect stderr to preserve these diagnostics. Any conformance violation, transform execution error, or unavailable selected dataset fails the audit. `--limit`, parser length filtering, or a dataset selection still narrows coverage and does not establish a complete configured-corpus audit.
