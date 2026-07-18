# texform-transform

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

A phase-oriented AST rewrite pipeline for TeXForm. It normalizes a parsed `Ast` into a canonical form so downstream consumers — formula equivalence comparison, MER tokenization, LLM pretraining corpora, polished authoring output — can work against a stable shape without re-implementing LaTeX semantics per use case. This README is the in-depth reference for the transform subsystem: rule authors and contributors should start here.

The crate is a thin wrapper around five ordered phases. Callers choose a build-time [`Profile`] / [`BuildConfig`] to compile a rewrite plan, then use per-run [`TransformConfig`] values to gate phases and set runtime limits.

## Quick start

```rust
use texform_core::parse::{ParseConfig, ParseContext};
use texform_transform::{BuildConfig, Profile, TransformContext};

let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
let mut ast = parse_ctx
    .parse_to_ast(r"\frac{a}{b}", &ParseConfig::default())
    .expect("source should parse");

// Pick a profile. `Faithful` preserves layout while expanding commands; use
// `Corpus` for complete canonical labels, `Equiv` for equivalence comparison, and
// `Authoring` for author-facing output.
let context = TransformContext::from_build_config(
    BuildConfig::profile(Profile::Faithful),
    &parse_ctx,
)
.expect("transform context should build");

let report = context
    .run(&mut ast, &parse_ctx)
    .expect("transform should succeed");

println!("rewrite iterations: {}", report.rewrite.iterations);
println!(
    "flatten removed_empty: {}",
    report.flatten_groups.actions.removed_empty
);
```

For repeated transforms with the same configuration, build a context once and reuse it:

```rust
use texform_transform::{BuildConfig, Profile, TransformContext};

let context = TransformContext::from_build_config(
    BuildConfig::profile(Profile::Faithful),
    &parse_ctx,
)?;
for mut ast in batch {
    let _report = context.run(&mut ast, &parse_ctx)?;
}
```

## Public API

The crate's public surface is intentionally small:

| Item | Purpose |
|------|---------|
| `BuildConfig::profile(profile)` | Select build-time normalization levels and default runtime config. |
| `TransformContext::from_build_config(config, parse_ctx) -> Result<Self, TransformBuildError>` | Precompile the rewrite plan once for reuse across many ASTs. |
| `TransformContext::run(ast, parse_ctx)` | Execute the precompiled pipeline with the profile default runtime config. |
| `TransformContext::run_with(ast, parse_ctx, config)` | Execute the precompiled pipeline with per-run overrides. |
| `TransformConfig` | Runtime phase gates, FlattenGroups behavior, and max rewrite iterations. |
| `TransformReport` | Per-phase reports aggregated across the run. |
| `TransformError` / `TransformBuildError` | Build-time and run-time error types. |

The rewrite phase additionally re-exports `RewriteRule`, `RuleLevel`, `RuleLevelSet`, `RuleKey`, `RuleMeta`, `RuleFidelity`, `RuleTarget`, `RuleTargetKey`, `RuleTargetKind`, `Plan as RewritePlan` and related items for callers that need to introspect rules.

## Pipeline

`TransformContext::run` executes a fixed sequence of phases. Normalization levels are chosen when the context is built; each run may disable rewrite / lower attributes or choose different FlattenGroups and iteration settings through `TransformConfig`.

1. **LowerAttributes (pre)** — canonicalize declarative-scope commands (e.g. `\bf x`) and registered prefix wrappers (e.g. `\mathbf{x}`) into a single normal form.
2. **Rewrite** — apply the precompiled rewrite plan in a fixed-point loop, bounded by `max_iterations`.
3. **LowerAttributes (post)** — re-canonicalize attribute markers introduced by rewrite rules (some Authoring / Faithful rules emit prefix wrappers that need lowering again).
4. **FinalizeAst** — local AST cleanup that does not depend on rewrite metadata, currently merging adjacent `Prime` nodes produced by rewrite rules.
5. **FlattenGroups** — remove redundant explicit and implicit groups after the earlier phases have stabilized.

Phase order is fixed; only the per-phase flags are configurable.

## Configuration

### `TransformConfig`

```rust
pub struct TransformConfig {
    pub rewrite_enabled: bool,
    pub lower_attributes_enabled: bool,
    pub finalize_ast: FinalizeAstConfig,
    pub flatten_groups: FlattenGroupsConfig,
    pub max_iterations: usize,
}
```

### Profiles

Each profile selects cumulative build-time rule levels and supplies a default runtime config.

| Profile | Rule levels | `flatten_groups` | Target scenario |
| --- | --- | --- | --- |
| `Authoring` | `Authoring` | `STRICT` | Polished author-facing formatting; stylistic choices kept. |
| `Faithful` | `Authoring` + `Faithful` | `STRICT` | Render-faithful universal forms. |
| `Corpus` | `Authoring` + `Faithful` + `Corpus` | `STRUCTURAL_ONLY` | Complete canonical forms that remain suitable labels for the original formulas. |
| `Equiv` | `Authoring` + `Faithful` + `Corpus` + `Equiv` | `STRUCTURAL_ONLY` | Aggressive intermediates for equivalence comparison, including projections that discard visually salient choices. |

The current builtin registry has no `Equiv`-level rules, so `Corpus` and `Equiv` temporarily produce the same output. Their intended products remain different.

#### `RuleLevel`

Every rule belongs to exactly one ordered level. A rule's level is the first
profile that accepts the rule output as a suitable product; it is not inferred
from render fidelity.

| Level      | Intent |
|------------|--------|
| `Authoring` | Author-editable canonical syntax: legacy modernization, typo fixes, and alias canonicalization without collapsing legitimate notation choices. |
| `Faithful` | Render-faithful universal forms for compact, package-specific, or legacy input. |
| `Corpus` | Complete, stable canonical forms that remain valid training labels for the original formulas; only training-irrelevant presentation variants and specialized vocabulary may collapse. |
| `Equiv` | Output is only suitable as an equivalence-checking, deduplication, or fingerprint intermediate, not as a corpus label; it may discard visually salient presentation choices. |

Classify a rule by asking which profile first accepts its output, then declare
the rule's fidelity independently. `fidelity` may rule out profiles whose floor
it cannot meet, but a high-fidelity rule is not automatically a lower level.

`Reading` fidelity is necessary but not sufficient for `Corpus`. A Corpus output must remain a credible complete label for the original formula. If a rewrite materially removes size, stretch, placement, visual hierarchy, or a notation distinction useful for training, classify it as `Equiv` even when notation identity, reading order, and structural roles remain intact. `Equiv` is a use-level rather than an alias for `Math` fidelity, so `Equiv`/`Reading` is a valid and informative combination.

#### `RuleFidelity`

`fidelity` is the worst-case equivalence guarantee over the rule's declared input domain. It is ordered from least to most faithful: `Math < Reading < Render`.

| Fidelity | Guarantee |
| --- | --- |
| `Render` | Rendering is equivalent under the reference renderer. |
| `Reading` | Notation content, reading order, and structural roles are preserved; layout may change. |
| `Math` | Mathematical meaning is preserved over the declared domain; notation and rendering may change. |

`fidelity` is a metadata contract only. `texform-transform` runs no rendering
comparison; how a downstream validator interprets a fidelity level when comparing
rendered output is defined by that consumer, not in this crate.

`fidelity` must not fall below the rule's level floor:

| Level | Min fidelity |
| --- | --- |
| `Authoring` | `Reading` |
| `Faithful` | `Reading` |
| `Corpus` | `Reading` |
| `Equiv` | `Math` |

Do not add a second metadata field for ordinary behavior. If a rule has an
important gap between its worst case and usual samples, document that gap in the
rule's top-level comment.

### `FlattenGroupsConfig`

FlattenGroups removes structurally redundant `Explicit` and `Implicit` groups. The four core actions are:

| Action                  | Trigger                                                                                          |
|-------------------------|--------------------------------------------------------------------------------------------------|
| `removed_empty`         | Empty `GroupChild` (`{}`) is dropped.                                                            |
| `replaced_single_child` | Single-child `GroupChild` is replaced by its child.                                              |
| `inlined_multi_child`   | Multi-child `GroupChild` is spliced into its parent's child sequence.                            |
| `unwrapped_slot`        | Single-child group occupying an `Argument` / `ScriptSub` / `ScriptSup` / `Infix*` slot is unwrapped. |

These actions are the default behavior. Eleven configuration flags (ten independent **preserve guards** plus one sub-flag) gate the actions in specific contexts. Each preserve guard belongs to one of two categories:

- **Semantic guards** — disabling them changes script binding, environment cell boundaries, declarative scope, or infix scope. Both parsed semantics and rendered output change.
- **Spacing guards** — disabling them only affects atom-spacing and unary/binary classification context. Parsed semantics are unchanged; rendered output may differ by a thin space.

#### Preserve guards

| # | Field                                              | Category | Example              | What the guard preserves                                                                              |
|---|----------------------------------------------------|----------|----------------------|-------------------------------------------------------------------------------------------------------|
| 1 | `preserve_group_containing_declarative_command`    | Semantic | `{\bf x} y`          | Groups whose subtree contains a declarative command (e.g. `\cal`, `\bf`), to avoid leaking declarative scope into following siblings. |
| 2 | `preserve_group_in_script_base_slot`               | Semantic | `{ab}^2`             | Groups occupying a `ScriptBase` slot, to avoid changing which atom subscripts or superscripts attach to. |
| 3 | `preserve_group_inside_env_body`                   | Semantic | `\begin{matrix} {a} & b \end{matrix}` | All groups inside an environment body, to preserve cell boundaries and intra-cell spacing.  |
| 4 | `preserve_group_containing_infix`                  | Semantic | `{a \over b}`        | `GroupChild`s whose subtree contains an `\over`-style infix, to preserve the infix scope.            |
| 5 | `preserve_group_adjacent_to_command_like`          | Spacing  | `\cos{A}`, `{\int}`  | `GroupChild`s whose preceding sibling or first child is command-like.                                |
| 6 | `preserve_group_as_argument_of_command`               | Spacing  | `\overline{{\sum}}`  | Risky singleton groups directly used as arguments of commands, preserving one spacing boundary while still flattening redundant nesting. |
| 7 | `preserve_empty_group`                             | Spacing  | `{}`                 | Empty `GroupChild`s, to preserve spacing / kerning effects.                                          |
| 8 | `preserve_group_with_lone_atom_spacing_char`       | Spacing  | `{+}`, `{,}`, `{*}_N`, `{·}m` | Singleton groups containing only one math atom-spacing character.                                    |
| 9 | `preserve_group_starting_with_atom_spacing_char`   | Spacing  | `{+x}`, `{,y}`       | Multi-child `GroupChild`s whose first child is a math atom-spacing character.                        |
| 10 | `preserve_group_containing_delimited_pair`        | Spacing  | `{\left( a \right)}` | `GroupChild`s whose subtree contains a `\left…\right` delimited group.                               |

Atom-spacing characters: `= < > + - , : ; . / * ! ? | ·`.

#### Sub-flag

| Field                                              | Depends on                                | Example     | Effect                                                                                                                                            |
|----------------------------------------------------|-------------------------------------------|-------------|---------------------------------------------------------------------------------------------------------------------------------------------------|
| `preserve_group_after_scripted_command_like`       | `preserve_group_adjacent_to_command_like` | `\sin^2{x}` | When classifying "command-like" for the adjacency check, recurse through `Scripted` bases. Disabled, `\sin^2` (a `Scripted` node) is no longer treated as command-like and the trailing `{x}` is flattened. |

This sub-flag does not gate any group on its own; it only refines the classification used by guard #5. When `preserve_group_adjacent_to_command_like` is `false`, the sub-flag has no effect.

#### Preset values

The preserve guards are wired to presets via two named constants:

- `FlattenGroupsConfig::STRICT` — all guards on. Used by `AUTHORING` and `FAITHFUL`.
- `FlattenGroupsConfig::STRUCTURAL_ONLY` — only semantic guards on; all spacing guards off. Used by `CORPUS` and `EQUIV`.

| Field                                              | Category | `AUTHORING` / `FAITHFUL` (STRICT) | `CORPUS` / `EQUIV` (STRUCTURAL_ONLY) |
|----------------------------------------------------|----------|:-------------------------------:|:------------------------------------------:|
| `enabled`                                          | –        | ✓                               | ✓                                          |
| `preserve_group_containing_declarative_command`    | Semantic | ✓                               | ✓                                          |
| `preserve_group_in_script_base_slot`               | Semantic | ✓                               | ✓                                          |
| `preserve_group_inside_env_body`                   | Semantic | ✓                               | ✓                                          |
| `preserve_group_containing_infix`                  | Semantic | ✓                               | ✓                                          |
| `preserve_group_adjacent_to_command_like`          | Spacing  | ✓                               | –                                          |
| `preserve_group_as_argument_of_command`               | Spacing  | ✓                               | –                                          |
| `preserve_group_after_scripted_command_like`       | Spacing  | ✓                               | –                                          |
| `preserve_empty_group`                             | Spacing  | ✓                               | –                                          |
| `preserve_group_with_lone_atom_spacing_char`       | Spacing  | ✓                               | –                                          |
| `preserve_group_starting_with_atom_spacing_char`   | Spacing  | ✓                               | –                                          |
| `preserve_group_containing_delimited_pair`         | Spacing  | ✓                               | –                                          |

Additional constants: `ENABLED` (alias for `STRICT`), `DISABLED` (no flattening at all), `DEFAULTS` (= `STRICT`).

## Reports

`TransformReport` aggregates per-phase reports for observability and diagnostics:

```rust
pub struct TransformReport {
    pub lower_attributes: LowerAttributesReport,
    pub rewrite: RewriteReport,
    pub finalize_ast: FinalizeAstReport,
    pub flatten_groups: FlattenGroupsReport,
}
```

- `LowerAttributesReport` — `attributes` (`HashMap<AttributeSet, AttributeStat>`) plus `eliminated_empty_segments`; each attribute stat has `consumed`, `redundant`, and `emitted` counts split into `declaratives` and `prefixes`. The report aggregates all LowerAttributes invocations in one transform run, so the default pipeline combines pre-Rewrite and post-Rewrite counts.
- `RewriteReport` — `iterations` (fixed-point iteration count) and `rules` (`Vec<RewriteRuleStat>` with `key`, `applied_count`, `skipped_count` per rule that was attempted at least once).
- `FinalizeAstReport` — `steps` with one `applied_count` counter per cleanup step (currently `merge_adjacent_primes`).
- `FlattenGroupsReport` — `actions` for the four action counters and `guards` for one hit counter per preserve guard. Hit counters are short-circuit: when several guards would apply to the same group, only the first one that matches in the internal evaluation order is incremented.

The stable facade DTO used by the Python and WebAssembly bindings flattens the same information into a transport-safe shape:

```text
{
  iterations,
  rules: [{ key, applied_count, skipped_count }],
  finalize_ast: {
    steps: { merge_adjacent_primes: { applied_count } }
  },
  flatten_groups: {
    actions: { removed_empty, replaced_single_child, inlined_multi_child, unwrapped_slot },
    guards: { preserve_group_* }
  },
  lower_attributes: {
    attributes: [{
      attr,
      value,
      consumed: { declaratives, prefixes },
      redundant: { declaratives, prefixes },
      emitted: { declaratives, prefixes }
    }],
    eliminated_empty_segments
  }
}
```

## Phase internals

### LowerAttributes

Two sub-modules drive this phase: `lower_attributes/codegen.rs` and build-time generated data emitted into `OUT_DIR`. The phase scans the AST for declarative commands (e.g. `\bf`, `\large`, `\sf`) and registered prefix wrappers (e.g. `\mathbf{...}`, `\textbf{...}`), then normalizes both forms into a single canonical representation per attribute.

Attributes are modeled as a structured `AttributeSet` (`Attr` × `AttrValue`) covering math font, math size, math style, text family, text series, text shape, and text size. Inherited state is tracked across container boundaries so that nested declarations, prefix wrappers, and empty trailing segments normalize cleanly.

The phase runs twice in the pipeline (pre and post Rewrite) under a single `enabled` switch because rewrite rules may emit prefix wrappers as their right-hand side; the post-pass re-canonicalizes those into the same normal form as the pre-pass. `LowerAttributesReport` uses a single cumulative counter set for both invocations.

### Rewrite

Rules live under `src/rewrite/rules/{base, ams, braket, physics}/` and are auto-registered through `src/rewrite/rules/generated.rs` (maintained by `build.rs`). Each rule is a unit struct implementing `RewriteRule` with a static `RuleMeta` descriptor.

`RuleMeta` is the static contract used to filter and order rules, invalidate them after runtime knowledge mutations, schedule fixed-point attempts, and check eliminated forms after the full pipeline. `TransformContext::from_build_config` compiles that metadata into a `Plan`; `scheduler::drive_fixed_point` then runs the plan until no rule applies or `max_iterations` is exceeded.

See [`src/rewrite/rules/README.md`](https://github.com/texform-dev/texform/blob/main/crates/texform-transform/src/rewrite/rules/README.md) for the authoritative metadata contract, including `triggers`, `eliminates`, `touches`, `produces`, dependency ordering, mutation filtering, convergence requirements, and the macro-based rule DSL.

### FinalizeAst

A single pass (`src/finalize_ast/`) for local AST cleanup that does not depend on rewrite metadata. Its current step merges adjacent `Prime` nodes produced by rewrite rules, so `f^{\prime\prime}` normalizes through `Prime(1), Prime(1)` into the same final shape as `f''`. The phase is enabled by default in every profile and gated by `TransformConfig.finalize_ast.enabled`.

### FlattenGroups

A single recursive traversal (`visit` → `try_unwrap` in `src/flatten_groups/mod.rs`). For each node the visitor:

1. Collects subtree-wide flags (`has_declarative`, `has_infix`, `has_delimited`) on the way down.
2. Tracks the `in_env_body` context flag through `Slot::EnvBody` edges.
3. On the way back up, calls `try_unwrap` to check whether the current group should be flattened. Each preserve guard short-circuits with an early return that increments its hit counter; the first matching guard wins.
4. If no guard fires and the group's content mode matches its parent's context mode, the group is unwrapped via either `unwrap_group_child` (multi-child splice) or `redirect_single_child_slot` (single-child slot replacement).

The `slot_can_unwrap` helper restricts redirect-style unwrapping to single-child groups in `Argument`, `Script*`, and `Infix*` slots; `EnvBody` slots are never unwrapped.

## Errors

```rust
pub enum TransformError {
    Build(TransformBuildError),
    Rewrite(RewriteError),
}

pub enum TransformBuildError {
    Rewrite(PlanBuildError),
}

pub enum RewriteError {
    Rule { rule: RuleKey, kind: RuleError },
    ContractViolation { target: RuleTargetKey, node_name: Option<String> },
    MaxIterationsExceeded { max_iterations: usize },
}

pub enum RuleError {
    InvalidNodeShape { message: String },
    MissingMetadata { name: String },
}
```

`TransformBuildError` is raised by `TransformContext::from_build_config` when the rewrite plan cannot be assembled, for example when a required package is missing. All other errors surface during execution.

## Tests

Integration tests cover the phases and their interactions:

- `tests/flatten_groups.rs` — preserve-guard toggles, `STRICT` vs `STRUCTURAL_ONLY`, action and per-guard counters.
- `tests/lower_attributes.rs` — declarative consumption, prefix wrapping, inherited-state absorption.
- `tests/finalize_ast.rs` — adjacent-`Prime` merging and the phase gate.
- `tests/rewrite_rule.rs` — single-rule execution and metadata contracts.
- `tests/rewrite_context.rs` — the `RuleContext` AST view exposed to rules.
- `tests/transform_contract.rs` — eliminated-form contract checking across the pipeline.
- `tests/config_model.rs` — profile and config invariants.

Run with:

```sh
cargo test -p texform-transform
```

## See also

- High-level overview: [repository README](https://github.com/texform-dev/texform#readme).
- Architecture: [`ARCHITECTURE.md`](https://github.com/texform-dev/texform/blob/main/ARCHITECTURE.md) (Transform Engine section).
- Rule authoring guide: [`src/rewrite/rules/README.md`](https://github.com/texform-dev/texform/blob/main/crates/texform-transform/src/rewrite/rules/README.md).
