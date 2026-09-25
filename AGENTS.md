# Agent Development Guide

TeXForm is a LaTeX formula parser, serializer, and transform engine. See [README.md](README.md) for usage and [ARCHITECTURE.md](ARCHITECTURE.md) for crate responsibilities and API invariants.

## Development Rules

- Write documentation, comments, examples, and user-facing text in English. Keep Markdown paragraphs and list items on single lines; preserve semantic line breaks in structured content.
- Keep internal research and planning documents outside this open-source repository.
- `texform` is the only stability-guaranteed Rust crate. External integrations use the facade; other crates are implementation details.
- Return errors for invalid user input. Reserve panics for violated internal invariants; use `unwrap()` or `expect()` in library code only when the invariant is proven.
- Prefer concrete implementations and existing helpers. Add abstractions for established needs, and profile before optimizing.
- Comments should explain constraints, invariants, or trade-offs rather than restating code.

## Read by Task

Read the relevant guide before changing an area; unrelated guides need not be loaded.

| Task | Guide |
| --- | --- |
| Change code or tests | [TESTING.md](TESTING.md): test placement and required checks |
| Change crate boundaries, public APIs, pipeline, or tree invariants | [ARCHITECTURE.md](ARCHITECTURE.md); update it when the documented model changes |
| Change transform behavior or profiles | [Transform reference](crates/texform-transform/README.md) |
| Add or change rewrite rules or metadata | [Rule authoring](crates/texform-transform/src/rewrite/rules/README.md) |
| Run or investigate corpus regression | [Regression guide](regression/README.md) |
| Change Python bindings | [Python development](crates/texform-python/README.md) |
| Change WASM, npm, or TypeScript bindings | [WASM development](crates/texform-wasm/README.md) |
| Change the command-line tool or the `serve` protocol | [CLI guide](crates/texform-cli/README.md) |
| Release, version, or changelog maintenance | [RELEASING.md](RELEASING.md) |

Use Cargo for Rust, Bun for JavaScript/TypeScript, and uv with maturin for Python. Significant parser changes require before/after corpus regression. Transform changes require the full `transform_contract` check before merging; hooks do not run it. Follow the commands and failure handling in the guides above.

Keep binding implementations, wrappers, and public type declarations in sync. Do not hand-edit `CHANGELOG.md` or add an `Unreleased` section during normal development; release maintenance follows `RELEASING.md`.

## Commit Messages

Use Conventional Commits: `<type>(<scope>)<!>: <subject>`. Use backticks around code identifiers in both subject and body.

- Use `feat` for user-facing features or behavior changes, `fix` for bug fixes, and `perf` for performance improvements. Supporting types are `docs`, `chore`, `ci`, `test`, `style`, `refactor`, `build`, `revert`. Mark breaking changes with `!`.
- Choose the type by the effect on released behavior. When changing something added since the last release, use `feat` or a supporting type instead of `fix`, and omit `!`: users never saw the earlier behavior, so the changelog should describe only the final result.
- Prefer an existing scope for the main change: `core`, `parser`, `serializer`, `document`, `transform`, `rule`, `specs`, `knowledge`, `argspec`, `interface`, `regression`, `bindings`, `python`, `wasm`, `cli`. Omit it when no area dominates. Use `core` only for shared internals or changes spanning several parts of `texform-core`.
- Keep the subject short, imperative, and lower-case after the prefix. State the main change without listing secondary changes.
- For nontrivial changes, explain the problem or motivation in a short opening paragraph, then describe the resulting behavior in unordered bullets grouped by topic. Use concrete declarative sentences to explain important changes and their boundaries; do not repeat the subject.
- For fixes, name the trigger and incorrect behavior. For breaking changes, retain the main migration instructions. Include meaningful performance or metric changes when relevant. Omit implementation inventories, exhaustive API lists, routine test summaries, and development history unless needed to explain a constraint or trade-off.

## Documentation Maintenance

Keep each detailed rule or procedure in its task guide; elsewhere, summarize and link. Use source doc comments for exact fields and implementation details, READMEs for usage and subsystem concepts, and `ARCHITECTURE.md` for relationships and guarantees.

When changing advertised knowledge-base totals, keep the repository and facade README descriptions consistent and coordinate the documentation-site headline update. Treat totals as coarse lower bounds; do not copy them into additional documents.
