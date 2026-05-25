# Testing

This document defines how TeXForm is tested: what each layer of tests is for, where tests live, how to write them, and where our confidence actually comes from. It complements the short "Testing and Validation" note in [`AGENTS.md`](AGENTS.md).

The guiding idea: **only the `texform` facade is a public, stability-guaranteed surface** (see `AGENTS.md` → Open-Source API Quality). Everything else is internal. Our test strategy follows directly from that boundary.

## Two Layers of Tests

We distinguish two kinds of tests by *intent*, not by mechanism.

**Contract tests** live only in the facade crate `texform` (`crates/texform/tests/`). They freeze the externally guaranteed behavior of the public API and are the compatibility promise we keep after open-sourcing. Treat them as load-bearing: a contract test changing its expectations means a public behavior changed, which is a deliberate, reviewable event. Write them by exercising the public API with real inputs and asserting observable results — not by pinning internal details. When the locked behavior is subtle, add a short comment naming the promise being protected.

**Implementation tests** live in the internal crates (`texform-core`, `texform-transform`, `texform-knowledge`, ...), either in `tests/` or inline. They verify that internal logic is correct. They carry **no external guarantee** and may be freely added, rewritten, or deleted as the implementation evolves. Do not treat them as a frozen interface.

The practical consequence: interface-freezing assertions belong in the facade. An internal crate should not contain tests whose only purpose is to pin down a detail "so it never changes" — that detail is not a promise we make.

## Where Tests Live

- **Inline `#[cfg(test)] mod tests`** for unit tests that need access to private items. Keep these close to the code they exercise.
- **`tests/`** for black-box integration tests that go through a crate's public API.
- **Prefer not to depend on private methods.** Reach for inline tests only when you genuinely need to lock an internal invariant; otherwise test through the public surface.
- Organize `tests/` **by behavior or subsystem**, not by mirroring `src/`. One test file may cover several source files, and one subsystem may be split across several test files. We do not maintain a 1:1 `src ↔ tests` mapping.
- "Which source isn't tested?" is answered by the coverage report (see below), not by a mirrored file layout.

## Naming and Authoring

- Name a test for the **behavior, condition, and expectation** it checks, e.g. `parse_unknown_command_preserves_node`.
- Follow arrange–act–assert. Keep each test focused on a single behavior; don't bundle unrelated assertions.
- Put shared setup in a `tests/support/` (or `common/`) module so the test body stays focused on input → assertion. `crates/texform-core/tests/support/mod.rs` is the reference example; extend the same pattern to other crates rather than duplicating setup.
- **Don't assert obvious constant values** (fixed strings, hard-coded identifiers) as if they were guarantees, especially in internal crates. Such tests pin trivia without verifying behavior.
- Don't over-specify: avoid assertions that lock formatting or structure unrelated to what the test is actually checking.

## Where Confidence Comes From

Coverage percentage is *not* our primary signal of quality. Confidence comes, in order, from:

1. **Corpus regression** — `texform-bench` runs the parser against large real-world datasets and tracks error rates over time. This is our closest analog to a conformance suite. See [`bench/README.md`](bench/README.md).
2. **Facade contract tests** — the public behavior the project promises.
3. **Snapshot tests** for parse/serialize/format output (recommended; see below).
4. **Fuzz regression corpus** for the lexer/parser (recommended; see below).

Any significant change to `texform-core` should be benchmarked before and after to check for regressions, as described in `AGENTS.md`.

## Recommended Techniques

These are encouraged where they fit; they are not mandatory across the board.

- **Snapshot testing (`insta`).** Parser ASTs, serializer output, and formatter results are natural fits — snapshots are easier to maintain than long hand-written `assert_eq!` chains. Caveat: snapshot tests may need to be skipped under `miri`; verify this if `miri` is introduced.
- **Fuzz regression corpus.** When fuzzing surfaces a crash or pathological input in the lexer/parser, check the minimized case into a regression corpus so it stays covered.

## Anti-Patterns

- Asserting obvious constant values instead of behavior.
- Repeated setup that buries the test in boilerplate instead of using a helper.
- Depending on private implementation details to make an external guarantee.
- Interface-freezing assertions inside an internal crate (treating an internal detail as a public promise).
- One test covering several unrelated concerns.
- Assertions that lock formatting or structure irrelevant to the behavior under test.
- Writing tests with no verification value just to raise a coverage number.

## Good Tests

- Focus on one behavior with clear inputs.
- Reuse helpers so the body is input → assertion.
- Cover the happy path, important edge cases, and regressions introduced by the change.
- For contract tests: stable, and commented with the public promise they protect.

## Coverage

Coverage is a **secondary signal**, used to find untested code — not a gate.

- Use `cargo-llvm-cov` for local inspection. Read it as **line coverage**; treat region coverage as observation only.
- There is **no CI coverage threshold**. We do not block merges on a percentage, and we do not chase a number.
- **Never write meaningless tests to raise coverage.** A lower number with honest tests is better than a high number padded with trivia.
- Exclude code that line coverage cannot meaningfully measure, via `cargo llvm-cov --ignore-filename-regex`:
  - FFI bindings (`texform-python`, `texform-wasm`) — exclude from core Rust coverage; validate them with binding-level Python/JS integration tests when those are added.
  - The procedural-macro crate (`texform-knowledge-macros`) and `trybuild` UI tests.
  - Generated code (`generated.rs`, e.g. `texform-knowledge`'s `builtin/generated.rs`).
