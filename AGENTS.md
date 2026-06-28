# Agent Development Guide

## Project Overview

TeXForm is a LaTeX formula parser, formatter, and transform engine. Code, documentation, comments, examples, and user-facing text must be clear, polished, and written in English.

See @README.md for usage, and @ARCHITECTURE.md for the architectural overview: crate layout, the processing pipeline, the three tree representations, and the public API guarantees.

## Repository Structure

```
crates/                       # Rust workspace
├── texform/                  # Public facade — the only stability-guaranteed crate
├── texform-core/             # Parser, AST, Document, serializer
├── texform-transform/        # Profile-based AST transform engine
├── texform-knowledge/        # Knowledge base & command specifications
├── texform-argspec/          # xparse-style argument spec parser
├── texform-knowledge-macros/ # Procedural macros for specs
├── texform-interface/        # Shared types (SyntaxNode, etc.)
├── texform-regression/       # Corpus regression harness
├── texform-python/           # Python bindings (PyO3)
└── texform-wasm/             # WebAssembly bindings
packages/                     # NPM/TypeScript packages
└── texform/                  # Public npm package wrapper around WASM bindings
python/texform/               # Python package source
crates/texform-knowledge/resources/specs/ # Knowledge base YAML
regression/                   # Corpus regression data & results
├── data/                     # Git LFS Parquet datasets
├── datasets.yaml             # Dataset configuration
├── contract_exceptions.yaml  # Transform contract allow-list
└── results/                  # Regression output
```

Project design notes and internal planning documents live outside this open-source repository. Do not recreate a `docs/` directory here for private notes, research records, or internal implementation plans.

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
- Keep private assumptions, unfinished design notes, and internal workflows out of this repo.

3. **Pragmatic Implementation**

- Prefer simple, concrete code. Add abstractions when they remove real duplication or clarify an established boundary.
- Profile before optimizing, and keep modules compact until splitting improves readability.

4. **Testing and Validation**

- See [`TESTING.md`](TESTING.md) for the full testing guide: the contract/implementation two-layer model, test layout, authoring conventions, and coverage policy.
- Contract tests (the public-API behavior we guarantee) live in the `texform` facade; internal crates carry implementation tests, inline or in `tests/`, with no external guarantee.
- Cover the happy path, important edge cases, and regressions introduced by the change.

## Corpus Regression

- The corpus regression suite in `crates/texform-regression` runs TeXForm against large real-world datasets. A full parser regression run takes less than 10 seconds on the canonical corpus.
- Current regression binaries are `parser_regression`, `transform_contract`, and `counter_dump`. `parser_regression` checks parser error-rate regressions against tracked summaries; `transform_contract` checks full-pipeline eliminated-form contracts over real corpora; `counter_dump` produces counter-map data products for downstream corpus analysis.
- Any significant parser change should run `parser_regression` before and after to check for error-rate regressions.
- `transform_contract` is not part of the pre-commit hook. Run it manually when changing transform rules, `RuleMeta`, `consumes.eliminates`, `touches`, `produces`, transform profiles/build config, rewrite scheduling, shared transform helpers, or `regression/contract_exceptions.yaml`.
- During development, a focused probe is acceptable first: `cargo run --release -p texform-regression --bin transform_contract -- --dataset lf80m-benchmarks --dry-run`. Before merging transform-related changes, run the full check: `cargo run --release -p texform-regression --bin transform_contract -- --dry-run`.
- New unlisted `transform_contract` violations must be triaged from `regression/results/transform_contract/commits/<hash>[-dirty]/violations.jsonl` before changing the allow-list. Do not add broad or unexplained exceptions.
- Historical parser summaries are tracked in git under `regression/results/parser_regression/`. There is no need to record baselines manually; diff the error rates before and after your change.
- See `./regression/README.md` for dataset details and result format.

## Transform Engine

The transform subsystem (`crates/texform-transform`) provides profile-based AST rewriting. See [`crates/texform-transform/README.md`](crates/texform-transform/README.md) for the subsystem reference and [`crates/texform-transform/src/rewrite/rules/README.md`](crates/texform-transform/src/rewrite/rules/README.md) for rule authoring conventions.

## Tooling Conventions

- **Rust**: `cargo test`, `cargo check`, `cargo clippy`; pre-commit hooks run Rust formatting, clippy, and parser regression checks. Transform-related changes must run `transform_contract` manually as described above.
- **TypeScript**: use `bun` as package manager.
- **Python**: use `uv` for dependency management and `maturin` for native extension builds.

### Python Binding

```bash
uv sync --dev          # set up .venv and install deps
uv run maturin develop # build the native extension from the repo root
```

### WASM Binding

```bash
wasm-pack build crates/texform-wasm --target nodejs
wasm-pack build crates/texform-wasm --target web
```

Or rebuild both targets and sync them into the npm package in one step:

```bash
bun run --cwd packages/texform prepare:publish
```

## Commit Messages

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>)<!>: <subject>`. The `type` and the optional `!` breaking marker drive the automatically generated changelog (see the Changelog note below), so choose them deliberately.

### Types

- `feat` — a user-facing feature or behavior change (grouped under **Added**).
- `fix` — a bug fix (grouped under **Fixed**).
- `perf` — a performance improvement (grouped under **Changed**).
- `docs`, `chore`, `ci`, `test`, `style`, `refactor`, `build`, `revert` — supporting changes; these are omitted from the changelog.

Append `!` after the type/scope to mark a breaking change, e.g. `feat(transform)!: ...`.

### Scopes

Prefer an existing scope naming the affected area: `core`, `parser`, `serializer`, `document`, `transform`, `rule`, `specs`, `knowledge`, `argspec`, `interface`, `regression`, `bindings`, `python`, `wasm`. Omit the scope only when a change genuinely spans multiple areas.

`core` is deliberately broad — the `texform-core` crate covers the parser, the AST, `Document`, and the serializer. Prefer a more specific scope for the part you actually touch (`parser`, `serializer`, `document`, ...) and reserve `core` for changes that genuinely span several of them or that sit in shared internals with no narrower home.

### Subject

Keep the subject short, imperative, and lower-case after the type/scope prefix. Use backticks around code identifiers — commands, types, methods — in the subject:

```bash
git commit -m 'feat(bindings): add `TransformEngine.transform`'
git commit -m 'feat(transform)!: reject foreign documents in `TransformEngine::transform`'
git commit -m 'fix(parser): preserve unknown `\left...\right` spans'
```

### Body

Whenever a commit is more than a trivial one-liner, add a body after a blank line that explains the change beyond what the subject already says. A good body covers three things:

- **Motivation** — why the change is needed: the bug's wrong behavior, the missing capability, or the constraint being satisfied. For a fix, give the concrete trigger and the incorrect result so the before/after is unambiguous.
- **What changed** — the mechanism or approach, described at the level of modules and concepts rather than restating the diff.
- **Why it matters** — the resulting behavior or guarantee, when it is not already obvious from the subject.

Write the body as short prose, or as a Markdown unordered list with one bullet per important change when a commit carries several. Only truly trivial commits (typo fixes, mechanical renames) may keep the subject alone.

## Maintenance Notes

### Changelog

[`CHANGELOG.md`](CHANGELOG.md) is generated automatically; do not write it by hand and do not add an `Unreleased` section during normal development. `release-plz` derives the changelog from Conventional Commit messages and maintains it inside the release PR it opens — that PR is where unreleased changes accumulate. This is why commit `type`, `scope`, and the `!` breaking marker matter: they decide whether a change appears in the changelog and how it is grouped (`feat` → Added, `fix` → Fixed, `perf` → Changed; `docs`/`chore`/`refactor`/etc. are omitted). Entries are polished, and binding/wrapper-only changes added, during release review (see [`RELEASING.md`](RELEASING.md)).

Versions are lockstep: one version number covers the Rust, Python, and JavaScript release channels.

### Headline Numbers

The headline numbers (such as "530+ command and environment specifications across 7 LaTeX packages") appear in exactly three places: the description sentence in the repository `README.md`, the description sentence in `crates/texform/README.md`, and the documentation site landing page (maintained separately). They are coarse lower bounds on the actual data in `texform-knowledge`. When the knowledge base or rule set crosses a milestone worth advertising, update all three together; do not introduce these numbers into other documents — use mechanism-level evidence (renderer names, text idempotency, corpus regression) instead.

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
