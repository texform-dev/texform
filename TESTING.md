# Testing

Choose checks by the behavior affected. This guide owns test placement and validation requirements; [regression/README.md](regression/README.md) owns corpus commands and result handling. Run commands from the repository root.

## Test Placement

| Behavior | Location and purpose |
| --- | --- |
| Stable Rust API | `crates/texform/tests/`: contract tests through the public facade |
| Internal implementation | Internal crate `tests/`, or inline tests when private access is needed |
| Individual rewrite rule | Inline `transform_examples!` golden tests and focused edge cases in the rule file |
| Phase scheduling, guards, and rule interactions | `crates/texform-transform/tests/` |
| Python conversion and exception behavior | Embedded Python tests in `crates/texform-python/src/` |
| JavaScript runtime behavior | `packages/texform/scripts/smoke-node.mjs` against rebuilt WASM |
| Construction and error behavior shared by Python and JavaScript | `crates/texform/tests/binding_cases.json`, run by both binding suites |
| TypeScript declarations | `packages/texform/type-tests/` and the package type check |
| Command-line behavior and the `serve` protocol | `crates/texform-cli/tests/`: integration tests that drive the built `texform` binary |

Facade contract tests define compatibility promises; changing their expectations requires a deliberate public behavior change. Internal tests verify correctness without freezing internal APIs. Binding tests protect host-language behavior that Rust facade tests cannot exercise.

Rule golden tests check implementation against the rule definition; they do not independently establish that the definition preserves rendering or meaning. Rule correctness also requires review and corpus validation. Put individual rule regressions in rule or phase tests rather than expanding facade tests with internal cases.

## Writing Tests

- Name the behavior, condition, and expected result. Use clear inputs and observable assertions; cover relevant happy paths, boundaries, and regressions.
- Organize integration tests by behavior or subsystem, not by mirroring source files. Use inline tests for private invariants and shared support modules for repeated setup.
- Keep each test focused. Avoid constant-value assertions, unrelated formatting checks, and tests that merely reproduce the implementation.
- Explain subtle contract guarantees briefly. Do not add tests solely to raise coverage or impose a test-first workflow on trivial changes.

## Required Checks

For Rust changes, run focused crate tests during development. Before handing off changes spanning crates or public behavior, run the workspace suite and the Rust checks used by CI:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
```

Additional checks depend on the change:

| Change | Required validation |
| --- | --- |
| Significant parser behavior | Run corpus regression before and after; compare parser error rates and investigate changes |
| Transform rules, metadata (`triggers`, `consumes`, `produces`, level or fidelity), profiles/build config, phase or rewrite scheduling, shared helpers, or contract exceptions | Run the full `transform_contract` across configured datasets before merging; a focused probe is sufficient only during development. Also run the [round-trip report](regression/README.md#round-trip-report) and state its changes |
| Python API, conversion, or packaging | Follow [Python development](crates/texform-python/README.md), including embedded tests and extension smoke check |
| WASM API, shared DTOs, npm wrapper, or TypeScript declarations | Follow [WASM development](crates/texform-wasm/README.md), including rebuild, type check, and Node smoke test |
| Documentation only | Check local links, command accuracy, and `git diff --check`; executable suites are needed only if behavior is also changed |

Corpus failures need investigation before baseline or allow-list changes. A transform execution error or an unlisted eliminated-form violation fails the transform gate. Do not add broad or unexplained exceptions. Ensure the intended datasets actually ran; a successful process exit alone does not establish full corpus coverage. See the [regression guide](regression/README.md) for diagnostic reruns.

## Structural Round Trips

The transform output contract is the transformed-tree round trip: the output text reparses to the transformed tree, in both output styles, up to representation-only differences such as an argument slot's own braces. It rests on two properties. Parsing and serialization round-trip every tree the parser can produce, and the transform produces only such trees. The transform's own fixed point then gives text idempotency as a corollary. Text idempotency alone is a weaker check: once the parser and serializer agree, reserializing a reparsed output reproduces the text even when the reparse built a different tree.

The shared checks in `texform_transform::round_trip` compare canonical trees directly:

| Check | Property |
| --- | --- |
| `parser_round_trip` | Parsing and serialization round-trip the parsed input, and the text is stable |
| `reparse` | The output text reparses to the transformed tree |
| `reparse_error` | The output text parses without errors (every such failure is also a `reparse` failure) |
| `retransform` | Transforming the transformed tree again leaves it unchanged |
| `reparse_retransform` | Transforming the reparsed output gives the transformed tree and the same text |

Inputs whose parse is incomplete are skipped. Two parts use these checks:

- **Fast round-trip tests** (`cargo test -p texform-transform --test reparse_roundtrip`) run generated and handwritten inputs, boundary examples, and a stratified subset of the adversarial boundary set under the `default`, `physics`, and `mixed` package sets, all four profiles, and both styles. They belong to `cargo test`: any failure is a test failure, and so is an unexpected transform error. The attribution test in the same file runs each `Render` rule alone, LowerAttributes where it preserves rendering, and FlattenGroups under `STRICT` on the adversarial inputs, and fails when boundary separation was needed. A unit test in `argument_capture.rs` checks on the same adversarial subset that every separator is needed: without it, or with an empty group in place of a wrapping group, the output reparses differently.
- **The round-trip report** (`transform_contract --round-trip-report`) runs the same checks on every configured dataset and the full adversarial set, and counts failures and skipped inputs per cell. It is an observation, not a merge condition. Transform changes should run it and state the per-cell changes, including transform failures, in the change description; rising counts need investigation, but the report never fails a check. See [Round-Trip Report](regression/README.md#round-trip-report).

## Hooks and CI

[Pre-commit hooks](.pre-commit-config.yaml) run Rust formatting, clippy, and parser regression refresh for Rust changes. Refresh may update tracked parser summaries; review those changes. Hooks do not run the unit-test suite or `transform_contract`.

[CI](.github/workflows/ci.yml) runs the Rust checks above, verifies the parser probe dataset against its tracked baseline, builds and imports a Python wheel, and rebuilds WASM for TypeScript and Node checks. The Python wheel smoke check verifies installation and parser construction, not the full Python API. The fast round-trip tests run as part of the workspace tests. The full transform corpus check and the round-trip report remain manual.

## Coverage

Use `cargo-llvm-cov` line coverage to locate untested behavior; coverage is not a merge gate and has no percentage target. Region coverage is observational only.

Exclude FFI bindings, procedural macros and `trybuild` tests, and generated code from core Rust coverage using `--ignore-filename-regex`. Validate bindings with their dedicated tests instead. Add meaningful assertions for uncovered behavior, not tests designed only to increase the number.
