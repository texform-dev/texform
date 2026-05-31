# Agent Development Guide

## Project Overview

TeXForm is a LaTeX formula parser, formatter, and transform engine. This repository is intended to be open source, so code, documentation, comments, examples, and user-facing text must be clear, polished, and written in English.

See @README.md for usage, and @ARCHITECTURE.md for the architectural overview: crate layout, the processing pipeline, the three tree representations, and the public API guarantees.

## Repository Structure

```
crates/                       # Rust workspace
├── texform-core/             # Parser, AST, serializer, transform engine
├── texform-knowledge/            # Knowledge base & command specifications
├── texform-argspec/          # xparse-style argument spec parser
├── texform-knowledge-macros/     # Procedural macros for specs
├── texform-interface/        # Public types (SyntaxNode, etc.)
├── texform-regression/       # Corpus regression harness
├── texform-python/           # Python bindings (PyO3)
└── texform-wasm/             # WebAssembly bindings
packages/                     # NPM/TypeScript packages
├── texform/                   # Public npm package wrapper around WASM bindings
├── argspecs-validator/       # Argument spec validation & spec-test runner
└── tex-renderers/            # MathJax / KaTeX / XeTeX rendering adapters
python/texform/               # Python package source
resources/specs/              # Knowledge base YAML
regression/                   # Corpus regression data & results
├── data/                     # Git LFS Parquet datasets
├── datasets.yaml             # Dataset configuration
├── results/                  # Regression output
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

- `texform` is the only crate with a public stability guarantee. All other crates (`texform-core`, `texform-transform`, etc.) are internal: their APIs may change without notice, and external integration must go through the `texform` facade.
- Public APIs should have stable names, predictable behavior, clear error types, and runnable examples.
- Keep private workspace assumptions, unfinished design notes, and internal workflows out of this repo.

3. **Pragmatic Implementation**

- Prefer simple, concrete code. Add abstractions when they remove real duplication or clarify an established boundary.
- Profile before optimizing, and keep modules compact until splitting improves readability.

4. **Testing and Validation**

- See [`TESTING.md`](TESTING.md) for the full testing guide: the contract/implementation two-layer model, test layout, authoring conventions, and coverage policy.
- Contract tests (the public-API behavior we guarantee) live in the `texform` facade; internal crates carry implementation tests, inline or in `tests/`, with no external guarantee.
- Cover the happy path, important edge cases, and regressions introduced by the change.

## Corpus Regression

- The corpus regression suite in `crates/texform-regression` runs TeXForm against large real-world datasets. A full parser regression run takes less than 10 seconds on the canonical corpus.
- Current regression binaries are `parser_regression` and `counter_dump`. `parser_regression` checks parser error-rate regressions against tracked summaries; `counter_dump` produces counter-map data products consumed by the parent workspace.
- `transform_contract` is the planned transform corpus contract entry point defined by the parent workspace's testing/evaluation redesign; its implementation is handled with the transform validation design, not this naming pass.
- Any significant parser change should run `parser_regression` before and after to check for error-rate regressions.
- Historical parser summaries are tracked in git under `regression/results/parser_regression/`. There is no need to record baselines manually; diff the error rates before and after your change.
- See `./regression/README.md` for dataset details and result format.

## Transform Engine

The transform subsystem (`crates/texform-core/src/transform/`) provides rule-based AST rewriting. See `crates/texform-core/src/transform/rules/README.md` for more info.

## Tooling Conventions

- **Rust**: `cargo test`, `cargo check`, `cargo clippy`; pre-commit hooks run `cargo fmt`, `cargo clippy`, spec validation, and the parser regression check.
- **TypeScript**: use `bun` as package manager.
- **Python**: use `uv` for dependency management and `maturin` for native extension builds.
- **Commit messages**: use Conventional Commits, such as `feat(rule): add root-family rule`, `fix(core): generate standard transform rule modules`, or `test(core): restore brace transform examples`. Prefer an existing scope like `core`, `rule`, `specs`, `regression`, `wasm`, or `python`; omit the scope only when a change spans multiple areas. Keep the subject short, imperative, and lower-case after the type/scope prefix. For large commits with several important changes, include a body after a blank line; format that body as a Markdown unordered list, with one bullet per important change.

### WASM Binding

```bash
wasm-pack build crates/texform-wasm --target nodejs
wasm-pack build crates/texform-wasm --target web
```

## Maintenance Notes

### Architecture Document

[`ARCHITECTURE.md`](ARCHITECTURE.md) is the public architectural overview. Keep it current: whenever you change the crate layout, the public API surface, the processing pipeline, or the tree representations and their invariants, update `ARCHITECTURE.md` in the same change.

### TypeScript Type Declaration Sync

Public JavaScript/TypeScript API declarations are maintained in `packages/texform/types/index.d.ts`.
When modifying exported WASM shapes or `texform-interface/src/syntax_node.rs`, update those public
types and regenerate the WASM package before checking the npm wrapper.

Verify after changes:

```bash
bun run --cwd packages/texform prepare:publish
bun run --cwd packages/texform check
```
