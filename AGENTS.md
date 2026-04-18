# Agent Development Guide

## Project Overview

TeXForm is a LaTeX formula parser, formatter, and transform engine — an evolving open-source project.

See [README.md](./README.md) for usage examples, serialization API, and language bindings.

## Repository Structure

```
crates/                       # Rust workspace (8 crates)
├── texform-core/             #   Parser, AST, serializer, transform engine
├── texform-specs/            #   Knowledge base & command specifications
├── texform-argspec/          #   xparse-style argument spec parser
├── texform-argspec-macros/   #   Procedural macros for argspec
├── texform-interface/        #   Public types (SyntaxNode, etc.)
├── texform-bench/            #   Corpus benchmark harness
├── texform-python/           #   Python bindings (PyO3 → pytexform)
└── texform-wasm/             #   WebAssembly bindings
packages/                     # NPM/TypeScript packages (bun workspace)
├── playground/               #   Interactive WASM playground (Vite + React + Monaco)
├── argspecs-validator/       #   Argument spec validation & spec-test runner
└── tex-renderers/            #   MathJax / KaTeX / XeTeX rendering adapters
python/pytexform/             # Python package source (maturin build)
resources/specs/              # Knowledge base YAML (base, ams, physics, …)
bench/                        # Corpus benchmark data & results
├── data/                     #   Git LFS Parquet datasets
├── datasets.yaml             #   Dataset configuration
├── results/                  #   Benchmark output (overall.json, per-dataset summaries)
└── history/                  #   Per-commit snapshots (yyyy-mm-dd-<hash>/)
data/spec-tests/              # Spec test data & results
docs/                         # Documentation & design (Chinese)
```

### Language Conventions

- **Source code** (`crates/`, `packages/`): comments and identifiers in English
- **Documentation** (`docs/`): Chinese

## Core Principles

1. **Fail Fast, Handle Appropriately**

- Use `panic!` or `unwrap()` for programming errors and states that should be impossible
- Use `Result<T>` for user-facing errors that callers need to handle
- Don't over-engineer error recovery for internal code paths, but design public APIs with clear error contracts

2. **Pragmatic Engineering**

- Deliver core functionality first, iterate based on real usage
- Minimize unnecessary abstractions — prefer concrete implementations over premature generalization
- Don't repeat yourself — extract shared logic when duplication is real, not hypothetical
- No premature optimization; profile before optimizing
- Prefer single-file modules until complexity demands splitting

3. **Code Quality**

- **Concise and precise**: clear expression, minimal boilerplate
- **Comment policy**:
  - Code itself should express *what* through naming — do NOT restate the code in comments
  - Comments exist to explain **why**: design decisions, non-obvious constraints, rejected alternatives, correctness arguments
  - Doc comments describe function responsibility and key semantics — do not repeat what the signature already says
  - **Writing no comments is just as harmful as writing bad comments**
- **English only in source**: all code comments, identifiers, and inline documentation must be in English

4. **Testing Strategy**

- Place public-API tests in `tests/`; place internal-implementation tests inline
- Tests should serve as executable documentation
- Cover the happy path + key edge cases
- No exhaustive testing — prioritize fast validation

## Corpus Benchmarks & Regression Testing

The corpus bench in `crates/texform-bench` runs the parser against large real-world datasets (1.2M+ formulas). Benchmark data is tracked via Git LFS in `bench/data/`.

**Any significant change to `texform-core` should be benchmarked before and after** to check for regressions in error rate and performance. See [bench/README.md](./bench/README.md) for dataset details and result format.

- A full benchmark run takes ~10 seconds.
- Historical results are tracked in git (`bench/results/`). There is no need to record baselines manually — just diff the error rates before and after your change.

## Transform Engine

The transform subsystem (`crates/texform-core/src/transform/`) provides rule-based AST rewriting:

- **Rule registry**: build script auto-discovers rule files under `transform/rules/` — no manual registration needed
- **Authoring macros**: `define_rule!` for general rules, `alias_rule!` for simple command renaming
- **Builtin rule sets**: `Normalize` and `Mer`, selectable at runtime via `TransformContext`

See [crates/texform-core/src/transform/rules/README.md](./crates/texform-core/src/transform/rules/README.md) for rule authoring conventions.

## Tooling Conventions

- **Rust**: `cargo test`, `cargo check`, `cargo clippy`; pre-commit hooks run `cargo fmt` and `cargo clippy`
- **TypeScript**: `bun` as package manager; `bun run dev` starts the playground
- **Python**: `uv` for dependency management; `maturin` for building native extensions

### WASM Binding

```bash
wasm-pack build crates/texform-wasm --target nodejs    # Node.js
wasm-pack build crates/texform-wasm --target bundler   # webpack etc.
```

### Embedded Resources

Command specs (`resources/specs/*.yaml`) are embedded into the binary at compile time via `include_str!()`. The `.so` and `.wasm` artifacts are fully self-contained. Changes to spec files require recompilation.

## Maintenance Notes

### TypeScript Type Declaration Sync

`crates/texform-wasm/src/lib.rs` contains a manual `typescript_custom_section` for `SyntaxNode` types. **When modifying types in `texform-interface/src/syntax_node.rs`, you must update this section to match.** This is a tsify-next limitation across crate boundaries — see the comment in that file for details.

Verify after changes:

```bash
wasm-pack build crates/texform-wasm --target nodejs
cat crates/texform-wasm/pkg/texform_wasm.d.ts
```

## Available CLI Examples

Two CLI examples (`parse` and `validate_spec`) are available for quick inspection and debugging. See [README.md](./README.md) for usage.
