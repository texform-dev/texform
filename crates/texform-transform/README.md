# texform-transform

A phase-oriented AST rewrite pipeline for TeXForm. It normalizes a parsed `Ast` into a canonical form so downstream consumers — formula equivalence comparison, MER tokenization, LLM pretraining corpora, polished authoring output — can work against a stable shape without re-implementing LaTeX semantics per use case.

The crate is a thin wrapper around four ordered phases driven by a single [`TransformConfig`]. Callers pick a profile, optionally tweak individual sub-configs, and run the pipeline once over an `Ast` borrowed from `texform-core`.

## Quick start

```rust
use texform_core::parse::{ParseConfig, ParseContext};
use texform_transform::{TransformConfig, run};

let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
let mut ast = parse_ctx
    .parse_to_ast(r"\frac{a}{b}", &ParseConfig::default())
    .expect("source should parse");

// Pick a profile. `CORPUS` keeps layout information; use `EQUIV` for
// equivalence comparison, `AUTHORING` for author-facing output.
let config = TransformConfig::CORPUS;

let report = run(&mut ast, &parse_ctx, &config)
    .expect("transform should succeed");

println!("rewrite iterations: {}", report.rewrite.iterations);
println!("flatten removed_empty: {}", report.flatten_groups.removed_empty);
```

For repeated transforms with the same configuration, build a context once and reuse it:

```rust
use texform_transform::{TransformConfig, TransformContext};

let context = TransformContext::from_config(TransformConfig::CORPUS, &parse_ctx)?;
for mut ast in batch {
    let _report = context.run(&mut ast, &parse_ctx)?;
}
```

## Public API

The crate's public surface is intentionally small:

| Item | Purpose |
|------|---------|
| `run(ast, parse_ctx, config) -> Result<TransformReport, TransformError>` | One-shot transform; builds a context internally. |
| `TransformContext::from_config(config, parse_ctx) -> Result<Self, TransformBuildError>` | Precompile the rewrite plan once for reuse across many ASTs. |
| `TransformContext::run(ast, parse_ctx)` | Execute the precompiled pipeline. |
| `TransformConfig` | Top-level configuration with `AUTHORING` / `CORPUS` / `CORPUS_DROP` / `EQUIV` presets. |
| `TransformReport` | Per-phase reports aggregated across the run. |
| `TransformError` / `TransformBuildError` | Build-time and run-time error types. |

The rewrite phase additionally re-exports `RewriteRule`, `RuleClass`, `RuleClassSet`, `RuleSelection`, `RuleKey`, `RuleMeta`, `RuleSafety`, `RuleTarget`, `RuleTargetKey`, `RuleTargetKind`, `Plan as RewritePlan` and related items for callers that need to introspect or filter rules.

## Pipeline

`run` executes a fixed sequence of phases driven from `TransformConfig`. Each phase is independent and can be disabled by setting `enabled = false` on its sub-config.

1. **LowerAttributes (pre)** — canonicalize declarative-scope commands (e.g. `\bf x`) and registered prefix wrappers (e.g. `\mathbf{x}`) into a single normal form.
2. **Rewrite** — apply rewrite rules in a fixed-point loop, bounded by `rewrite.max_iterations`. Only rules whose class is selected in `rewrite.classes` and whose key passes `rewrite.selection` are eligible.
3. **LowerAttributes (post)** — re-canonicalize attribute markers introduced by rewrite rules (some Standard / Expand rules emit prefix wrappers that need lowering again).
4. **FlattenGroups** — remove redundant explicit and implicit groups after the earlier phases have stabilized.

Phase order is fixed; only the per-phase flags are configurable.

## Configuration

### `TransformConfig`

```rust
pub struct TransformConfig {
    pub lower_attributes: LowerAttributesConfig,
    pub rewrite: RewriteConfig,
    pub flatten_groups: FlattenGroupsConfig,
}
```

### Presets

Each preset bundles a sub-config for every phase to target a specific downstream scenario. Constructed as `const` values; clone and override as needed.

| Preset        | `lower_attributes` | `rewrite.classes`                        | `flatten_groups`    | Target scenario                                              |
|---------------|--------------------|------------------------------------------|---------------------|--------------------------------------------------------------|
| `AUTHORING`   | `ENABLED`          | `Standard`                               | `STRICT`            | Polished author-facing formatting; stylistic choices kept.   |
| `CORPUS`      | `ENABLED`          | `Standard` + `Expand`                    | `STRICT`            | MER input or LLM pretraining corpus; layout info preserved.  |
| `CORPUS_DROP` | `ENABLED`          | `Standard` + `Expand` + `Drop`           | `STRUCTURAL_ONLY`   | Stronger corpus cleaning; drops linebreak/layout hints.      |
| `EQUIV`       | `ENABLED`          | `Standard` + `Expand` + `Drop` + `Equiv` | `STRUCTURAL_ONLY`   | Aggressive normalization for formula equivalence comparison. |

### `LowerAttributesConfig`

| Field     | Type   | Default | Notes                                           |
|-----------|--------|---------|-------------------------------------------------|
| `enabled` | `bool` | `true`  | Shared by the pre-pass and post-pass invocations. |

Constants: `ENABLED`, `DISABLED`, `DEFAULTS` (= `ENABLED`).

### `RewriteConfig`

```rust
pub struct RewriteConfig {
    pub enabled: bool,
    pub classes: RuleClassSet,
    pub max_iterations: usize,
    pub selection: RuleSelection,
}
```

| Field            | Type             | Default        | Notes                                                              |
|------------------|------------------|----------------|--------------------------------------------------------------------|
| `enabled`        | `bool`           | `true`         | When `false`, the rewrite plan is not built and the phase is skipped. |
| `classes`        | `RuleClassSet`   | empty          | Bit set of `RuleClass` values; rule must belong to a selected class. |
| `max_iterations` | `usize`          | `100`          | Upper bound on fixed-point iterations; exceeding it is an error.   |
| `selection`      | `RuleSelection`  | `All`          | `All`, `Only(Vec<RuleKey>)`, or `Except(Vec<RuleKey>)`.            |

Helper methods on `RewriteConfig`:

- `only(key)` / `only_many(keys)` — narrow the selection to specific rules.
- `disable(key)` / `disable_many(keys)` — exclude specific rules.

#### `RuleClass`

Every rule belongs to exactly one class. The class captures the rule's intent, not its mechanism:

| Class      | Intent |
|------------|--------|
| `Standard` | Uncontroversial author-facing standardization: deprecated-syntax modernization, typo fixes, alias canonicalization, cross-package anchor unification. Does not collapse stylistic choices that an author may legitimately make. |
| `Expand`   | Corpus-oriented normal form: rewrites convenience commands, semantic macros, package-specific commands, and spacing primitives into more universal structures. Output remains readable LaTeX and preserves layout information. |
| `Drop`     | Removes non-ink, metadata, and layout hints a corpus should not learn — linebreak preferences, invisible layout nodes, and similar caller-opt-in deletions. |
| `Equiv`    | Aggressive normalization tuned for equivalence comparison; may sacrifice common idioms or author intent for higher recall. Rewrites rather than deletes. |

A rule's class is decided by its immediate rewrite intent, not by what later rules might do to the result.

#### `RuleSafety`

Orthogonal to class: how much information the rule preserves.

| Safety        | Meaning                                                                   |
|---------------|---------------------------------------------------------------------------|
| `Lossless`    | Fully reversible; no information lost.                                    |
| `Semantic`    | Mathematical meaning preserved; non-semantic detail may be discarded.     |
| `Destructive` | May lose information that affects rendering or meaning.                   |

Safety is informational — it is not used by the scheduler but is exposed via `RuleMeta` for diagnostics, dependency analysis, and rule-set construction.

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

- `FlattenGroupsConfig::STRICT` — all guards on. Used by `AUTHORING` and `CORPUS`.
- `FlattenGroupsConfig::STRUCTURAL_ONLY` — only semantic guards on; all spacing guards off. Used by `CORPUS_DROP` and `EQUIV`.

| Field                                              | Category | `AUTHORING` / `CORPUS` (STRICT) | `CORPUS_DROP` / `EQUIV` (STRUCTURAL_ONLY) |
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
    pub flatten_groups: FlattenGroupsReport,
}
```

- `LowerAttributesReport` — per-attribute counters: `consumed`, `collapsed`, `wrapped`, `reinserted`, `eliminated_empty_segments`, `absorbed_prefixes`.
- `RewriteReport` — `iterations` (fixed-point iteration count) and `applied` (`Vec<AppliedRuleStat>` with `key`, `count`, `skipped_count` per rule that was attempted at least once).
- `FlattenGroupsReport` — four action counters plus one hit counter per preserve guard. Hit counters are short-circuit: when several guards would apply to the same group, only the first one that matches in the internal evaluation order is incremented.

## Phase internals

### LowerAttributes

Two sub-modules drive this phase: `lower_attributes/codegen.rs` and the build-time-generated `lower_attributes/generated.rs`. The phase scans the AST for declarative commands (e.g. `\bf`, `\large`, `\sf`) and registered prefix wrappers (e.g. `\mathbf{...}`, `\textbf{...}`), then normalizes both forms into a single canonical representation per attribute.

Attributes are modeled as a structured `Attr` × `AttrValue` × `ContentMode` tuple covering math font, math size, math style, text family, text series, text shape, and text size. Inherited state is tracked across container boundaries so that nested declarations collapse when redundant and segments empty out cleanly.

The phase runs twice in the pipeline (pre and post Rewrite) under a single `enabled` switch because rewrite rules may emit prefix wrappers as their right-hand side; the post-pass re-canonicalizes those into the same normal form as the pre-pass.

### Rewrite

Rules live under `src/rewrite/rules/{base, ams, braket, physics}/` and are auto-registered through `src/rewrite/rules/generated.rs` (produced by `build.rs`). Each rule is a unit struct implementing `RewriteRule` with a static `RuleMeta` descriptor.

`RuleMeta` declares:

- `key: RuleKey` (`package/name`) — the stable identifier used in `RuleSelection::Only` / `Except`.
- `enabled_by_packages` — packages whose presence in the `ParseContext` enables the rule.
- `class` — see [`RuleClass`](#ruleclass).
- `safety` — see [`RuleSafety`](#rulesafety).
- `triggers` — `RuleTarget`s the scheduler watches to know when to attempt the rule.
- `consumes` — forms the rule `eliminates` (must not appear in the output) and `touches` (may read or modify).
- `produces` — forms the rule may introduce. The engine verifies every produced form is either accepted by the output contract or eliminated by another rule.

`TransformContext::from_config` builds a `Plan` by filtering all registered rules through `(rewrite.classes, rewrite.selection)` and the `ParseContext`'s enabled packages. The plan is then driven by `scheduler::drive_fixed_point` until either no rule fires in an iteration or `max_iterations` is exceeded. After the loop, `contract::assert_eliminated_forms` verifies that no rule's `eliminates` set remains in the output AST; a violation is reported as `RewriteError::ContractViolation`.

See [`src/rewrite/rules/README.md`](src/rewrite/rules/README.md) for rule authoring conventions and the macro-based DSL used to define rules.

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

`TransformBuildError` is raised by `TransformContext::from_config` when the rewrite plan cannot be assembled (for example, a `RuleSelection::Only` key references an unknown rule, or a required package is missing). All other errors surface during execution.

## Tests

Integration tests cover the phases and their interactions:

- `tests/flatten_groups.rs` — preserve-guard toggles, `STRICT` vs `STRUCTURAL_ONLY`, action and per-guard counters.
- `tests/lower_attributes.rs` — declarative consumption, prefix wrapping, inherited-state absorption.
- `tests/rewrite_rule.rs` — single-rule execution and metadata contracts.
- `tests/rewrite_context.rs` — the `RuleContext` AST view exposed to rules.

Run with:

```sh
cargo test -p texform-transform
```

## See also

- High-level overview: [`lib/texform/README.md`](../../README.md) (Transform section).
- Rule authoring guide: [`src/rewrite/rules/README.md`](src/rewrite/rules/README.md).
- FlattenGroups guard categorization and analysis design: internal design memo `2026-05-20-flatten-groups-guards-refactor-design.md` (workspace).
