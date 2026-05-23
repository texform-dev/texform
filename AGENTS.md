# Agent Development Guide

## Project Overview

TeXForm is a LaTeX formula parser, formatter, and transform engine. This repository is intended to be open source, so code, documentation, comments, examples, and user-facing text must be clear, polished, and written in English.

See @README.md for more about this project.

## Repository Structure

```
crates/                       # Rust workspace
├── texform-core/             # Parser, AST, serializer, transform engine
├── texform-specs/            # Knowledge base & command specifications
├── texform-argspec/          # xparse-style argument spec parser
├── texform-specs-macros/     # Procedural macros for specs
├── texform-interface/        # Public types (SyntaxNode, etc.)
├── texform-bench/            # Corpus benchmark harness
├── texform-python/           # Python bindings (PyO3)
└── texform-wasm/             # WebAssembly bindings
packages/                     # NPM/TypeScript packages
├── argspecs-validator/       # Argument spec validation & spec-test runner
└── tex-renderers/            # MathJax / KaTeX / XeTeX rendering adapters
python/texform/               # Python package source
resources/specs/              # Knowledge base YAML
bench/                        # Corpus benchmark data & results
├── data/                     # Git LFS Parquet datasets
├── datasets.yaml             # Dataset configuration
├── results/                  # Benchmark output
└── history/                  # Per-commit snapshots
data/argspec-validate-results/  # Spec validation results
```

Project design notes and internal planning documents live outside this repo in the parent workspace. Do not recreate a `docs/` directory here for private notes, Chinese research records, or internal implementation plans.

## Language Conventions

- Use English for all documentation, README content, examples, comments, doc comments, commit-facing messages in scripts, and user-facing text.
- Keep comments concise and useful: explain why something exists, what invariant matters, or what trade-off is being preserved. Do not restate the code.

## Core Principles

1. **Rust Error Handling**

- Avoid `unwrap()` and `expect()` in library code unless an invariant has already been proven. If `expect()` is appropriate, include a useful message.
- Reserve `panic!` for violated internal invariants, not normal input validation or caller-facing errors.

2. **Open-Source API Quality**

- Public APIs should have stable names, predictable behavior, clear error types, and runnable examples.
- Keep private workspace assumptions, unfinished design notes, and internal workflows out of this repo.

3. **Pragmatic Implementation**

- Prefer simple, concrete code. Add abstractions when they remove real duplication or clarify an established boundary.
- Profile before optimizing, and keep modules compact until splitting improves readability.

4. **Testing and Validation**

- Put public-API tests in `tests/` and focused implementation tests inline.
- Cover the happy path, important edge cases, and regressions introduced by the change.

## Corpus Benchmarks & Regression Testing

- The corpus bench in `crates/texform-bench` runs the parser against large real-world datasets. A full benchmark run takes less than 10 seconds.
- Current bench binaries are `parse_corpus`, `counter_dump`, and `evaluate_flatten_groups`. The first is the parser corpus regression bench, `counter_dump` produces counter-map data products, and `evaluate_flatten_groups` writes fixed FlattenGroups impact summaries under `bench/results/flatten_groups/` while also printing a short console summary.
- In the parent workspace vocabulary, "bench" means corpus regression with pass/regression semantics, while "eval" means repeatable corpus comparison without pass/fail semantics.
- Any significant change to `texform-core` should be benchmarked before and after to check for regressions in error rate and performance.
- Historical parser results are tracked in git under `bench/results/parse_corpus/`. There is no need to record baselines manually; diff the error rates before and after your change.
- See `./bench/README.md` for dataset details and result format.

## Transform Engine

The transform subsystem (`crates/texform-core/src/transform/`) provides rule-based AST rewriting. See `crates/texform-core/src/transform/rules/README.md` for more info.

## Tooling Conventions

- **Rust**: `cargo test`, `cargo check`, `cargo clippy`; pre-commit hooks run `cargo fmt`, `cargo clippy`, spec validation, and the bench regression check.
- **TypeScript**: use `bun` as package manager.
- **Python**: use `uv` for dependency management and `maturin` for native extension builds.
- **Commit messages**: use Conventional Commits, such as `feat(rule): add root-family rule`, `fix(core): generate standard transform rule modules`, or `test(core): restore brace transform examples`. Prefer an existing scope like `core`, `rule`, `specs`, `bench`, `wasm`, or `python`; omit the scope only when a change spans multiple areas. Keep the subject short, imperative, and lower-case after the type/scope prefix.

### WASM Binding

```bash
wasm-pack build crates/texform-wasm --target nodejs
wasm-pack build crates/texform-wasm --target bundler
```

## Maintenance Notes

### TypeScript Type Declaration Sync

`crates/texform-wasm/src/lib.rs` contains a manual `typescript_custom_section` for `SyntaxNode` types. When modifying types in `texform-interface/src/syntax_node.rs`, update this section to match.

Verify after changes:

```bash
wasm-pack build crates/texform-wasm --target nodejs
cat crates/texform-wasm/pkg/texform_wasm.d.ts
```
