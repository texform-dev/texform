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
| Transform rules, metadata (`triggers`, `consumes`, `produces`, level or fidelity), profiles/build config, phase or rewrite scheduling, shared helpers, or contract exceptions | Run the full `transform_contract` across configured datasets before merging; a focused probe is sufficient only during development |
| Python API, conversion, or packaging | Follow [Python development](crates/texform-python/README.md), including embedded tests and extension smoke check |
| WASM API, shared DTOs, npm wrapper, or TypeScript declarations | Follow [WASM development](crates/texform-wasm/README.md), including rebuild, type check, and Node smoke test |
| Documentation only | Check local links, command accuracy, and `git diff --check`; executable suites are needed only if behavior is also changed |

Corpus failures need investigation before baseline or allow-list changes. A transform execution error or an unlisted eliminated-form violation fails the transform gate. Do not add broad or unexplained exceptions. Ensure the intended datasets actually ran; a successful process exit alone does not establish full corpus coverage. See the [regression guide](regression/README.md) for diagnostic reruns.

## Structural Round Trips

`crates/texform-transform/tests/reparse_roundtrip.rs` checks parser and transform structure as well as output strings. To audit a prepared UTF-8 corpus containing one formula per line, run:

```bash
TEXFORM_ROUNDTRIP_CORPUS=/path/to/formulas.txt cargo test --release -p texform-transform --test reparse_roundtrip -- --ignored --nocapture
```

The audit explicitly loads the six default packages, so it does not depend on the library defaults.

## Hooks and CI

[Pre-commit hooks](.pre-commit-config.yaml) run Rust formatting, clippy, and parser regression refresh for Rust changes. Refresh may update tracked parser summaries; review those changes. Hooks do not run the unit-test suite or `transform_contract`.

[CI](.github/workflows/ci.yml) runs the Rust checks above, verifies the parser probe dataset against its tracked baseline, builds and imports a Python wheel, and rebuilds WASM for TypeScript and Node checks. The Python wheel smoke check verifies installation and parser construction, not the full Python API. The full transform corpus check remains manual.

## Coverage

Use `cargo-llvm-cov` line coverage to locate untested behavior; coverage is not a merge gate and has no percentage target. Region coverage is observational only.

Exclude FFI bindings, procedural macros and `trybuild` tests, and generated code from core Rust coverage using `--ignore-filename-regex`. Validate bindings with their dedicated tests instead. Add meaningful assertions for uncovered behavior, not tests designed only to increase the number.
