# texform-transform

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

A phase-oriented AST rewrite pipeline. This guide owns phase behavior, profile selection, and fidelity definitions. For rule metadata and authoring, use the [rule guide](src/rewrite/rules/README.md).

The crate runs four phase implementations in a fixed order, with LowerAttributes and FinalizeAst each invoked twice in the default pipeline. Callers choose a build-time `Profile` / `BuildConfig` to compile a rewrite plan, then use per-run `TransformConfig` values to gate phases and set runtime limits.

## Quick start

```rust
use texform_core::ast::Ast;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_transform::{BuildConfig, Profile, TransformContext};

let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
let (document, _) = parse_ctx
    .parse(r"\frac{a}{b}", &ParseConfig::default())
    .try_into_document()
    .expect("source should parse");
let mut ast = Ast::from_syntax_root(&document.to_syntax());

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

Build a context once and reuse it for repeated transforms with the same profile and knowledge base.

## Internal API

The crate's public surface is intentionally small:

| Item | Purpose |
|------|---------|
| `BuildConfig::profile(profile)` | Select build-time rule levels and default runtime config. |
| `TransformContext::from_build_config(config, parse_ctx) -> Result<Self, TransformBuildError>` | Precompile the rewrite plan once for reuse across many ASTs. |
| `TransformContext::run(ast, parse_ctx)` | Execute the precompiled pipeline with the profile default runtime config. |
| `TransformContext::run_with(ast, parse_ctx, config)` | Execute the precompiled pipeline with per-run overrides. |
| `TransformConfig` | Runtime phase gates, FlattenGroups behavior, and max rewrite iterations. |
| `TransformReport` | Per-phase reports aggregated across the run. |
| `TransformError` / `TransformBuildError` | Build-time and run-time error types. |

See [crate exports](src/lib.rs) for the internal Rust surface.

## Pipeline

`TransformContext::run` executes a fixed sequence of phases. Rule levels are chosen when the context is built; each run may disable Rewrite, LowerAttributes, FinalizeAst, or FlattenGroups, or choose different preserve guards and iteration settings through `TransformConfig`.

1. **LowerAttributes (pre)** — canonicalize declarative-scope commands (e.g. `\bf x`) and registered prefix wrappers (e.g. `\mathbf{x}`) into a single normal form.
2. **Rewrite** — apply the precompiled rewrite plan in a fixed-point loop, bounded by `rewrite.max_iterations`.
3. **LowerAttributes (post)** — re-canonicalize attribute markers introduced by rewrite rules (some Authoring / Faithful rules emit prefix wrappers that need lowering again).
4. **FinalizeAst** — profile-neutral AST canonicalization (adjacent `Prime` merges, text-sequence normalization). Runs before FlattenGroups so merges can create single-child groups.
5. **FlattenGroups** — remove redundant explicit and implicit groups after the earlier phases have stabilized.
6. **FinalizeAst** (again, when FlattenGroups is enabled) — the same idempotent pass, so adjacency exposed by flattening is canonicalized. FinalizeAst is the last phase that mutates the AST; eliminated-form contract validation that follows is read-only.

Phase order is fixed; only the per-phase flags are configurable. When `flatten_groups.enabled` is false, the engine skips the second FinalizeAst call because the first pass already finished AST mutation for that input.

## Configuration

### `TransformConfig`

Per-run configuration nests `lower_attributes`, `rewrite`, `finalize_ast`, and `flatten_groups`. Each phase has an enable flag; Rewrite also carries `max_iterations`. Start from the selected profile's defaults when changing individual options. Rust callers pass a complete `TransformConfig`. Bindings merge partial overrides onto a base config. See [config.rs](src/config.rs) for fields and constructors.

### Profiles

Each profile selects cumulative build-time rule levels and supplies a default runtime config.

| Profile | Rule levels | `flatten_groups` | Target scenario |
| --- | --- | --- | --- |
| `Authoring` | `Authoring` | `STRICT` | Polished author-facing formatting; stylistic choices kept. |
| `Faithful` | `Authoring` + `Faithful` | `STRICT` | Render-faithful universal forms. |
| `Corpus` | `Authoring` + `Faithful` + `Corpus` | `STRUCTURAL_ONLY` | Complete canonical forms that remain suitable labels for the original formulas. |
| `Equiv` | `Authoring` + `Faithful` + `Corpus` + `Equiv` | `STRUCTURAL_ONLY` | Aggressive intermediates for equivalence comparison, including projections that discard visually salient choices. |

#### `RuleLevel`

Every rule belongs to exactly one ordered level. A rule's level is the first profile that accepts the rule output as a suitable product; it is not inferred from render fidelity.

| Level      | Intent |
|------------|--------|
| `Authoring` | Author-editable canonical syntax: legacy modernization, typo fixes, and alias canonicalization without collapsing legitimate notation choices. |
| `Faithful` | Render-faithful universal forms for compact, package-specific, or legacy input. |
| `Corpus` | Complete, stable canonical forms that remain valid training labels for the original formulas; only training-irrelevant presentation variants and specialized vocabulary may collapse. |
| `Equiv` | Output is only suitable as an equivalence-checking, deduplication, or fingerprint intermediate, not as a corpus label; it may discard visually salient presentation choices. |

Classify a rule by asking which profile first accepts its output, then declare the rule's fidelity independently. `fidelity` may rule out profiles whose floor it cannot meet, but a high-fidelity rule is not automatically a lower level.

`Reading` fidelity is necessary but not sufficient for `Corpus`. A Corpus output must remain a credible complete label for the original formula. If a rewrite materially removes size, stretch, placement, visual hierarchy, or a notation distinction useful for training, classify it as `Equiv` even when notation identity, reading order, and structural roles remain intact. `Equiv` is a use-level rather than an alias for `Math` fidelity, so `Equiv`/`Reading` is a valid and informative combination.

#### `RuleFidelity`

`fidelity` is the worst-case equivalence guarantee over the rule's declared input domain. It is ordered from least to most faithful: `Math < Reading < Render`.

| Fidelity | Guarantee |
| --- | --- |
| `Render` | Rendering is equivalent under the reference renderer. |
| `Reading` | Notation content, reading order, and structural roles are preserved; layout may change. |
| `Math` | Mathematical meaning is preserved over the declared domain; notation and rendering may change. |

`fidelity` is a metadata contract only. `texform-transform` runs no rendering comparison; how a downstream validator interprets a fidelity level when comparing rendered output is defined by that consumer, not in this crate.

`fidelity` must not fall below the rule's level floor:

| Level | Min fidelity |
| --- | --- |
| `Authoring` | `Reading` |
| `Faithful` | `Reading` |
| `Corpus` | `Reading` |
| `Equiv` | `Math` |

Do not add a second metadata field for ordinary behavior. If a rule has an important gap between its worst case and usual samples, document that gap in the rule's top-level comment.

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
| 3 | `preserve_group_inside_env_body` | Semantic | `\begin{matrix} {a} & b \end{matrix}` | Groups inside an environment body, except a lone `Prime` in a superscript slot, to preserve cell boundaries and intra-cell spacing. |
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

`STRICT` enables all preserve guards; `STRUCTURAL_ONLY` keeps semantic guards and disables spacing guards. Profiles select them as shown above. `ENABLED` and `DEFAULTS` alias `STRICT`; `DISABLED` skips FlattenGroups. See [FlattenGroups configuration](src/flatten_groups/mod.rs) for exact fields and presets.

## Reports

[TransformReport](src/report.rs) contains one report per phase. LowerAttributes and FinalizeAst accumulate their multiple invocations in the same report; already-canonical nodes are not recounted by FinalizeAst. Rewrite records iterations and per-rule outcomes. FlattenGroups counts the first matching preserve guard for a group; when command adjacency is established through a scripted base, both `preserve_group_adjacent_to_command_like` and its `preserve_group_after_scripted_command_like` sub-flag are incremented.

Bindings use a transport DTO rather than the Rust report layout. Keep changes synchronized with [shared binding DTOs](../texform/src/bindings/mod.rs), [Python stubs](../../python/texform/__init__.pyi), and [TypeScript declarations](../../packages/texform/types/index.d.ts).

## Phase internals

### LowerAttributes

Two sub-modules drive this phase: `lower_attributes/codegen.rs` and build-time generated data emitted into `OUT_DIR`. The phase scans the AST for declarative commands (e.g. `\bf`, `\large`, `\sf`) and registered prefix wrappers (e.g. `\mathbf{...}`, `\textbf{...}`), then normalizes both forms into a single canonical representation per attribute.

Attributes are modeled as a structured `AttributeSet` (`Attr` × `AttrValue`) covering math font, math size, math style, text family, text series, text shape, and text size. Inherited state is tracked across container boundaries so that nested declarations, prefix wrappers, and empty trailing segments normalize cleanly.

The phase runs twice in the pipeline (pre and post Rewrite) under a single `enabled` switch because rewrite rules may emit prefix wrappers as their right-hand side; the post-pass re-canonicalizes those into the same normal form as the pre-pass. `LowerAttributesReport` uses a single cumulative counter set for both invocations.

### Rewrite

Rules live under `src/rewrite/rules/<package>/<level>/<group>/` and are auto-registered through `src/rewrite/rules/generated.rs` (maintained by `build.rs`). Each rule is a unit struct implementing `RewriteRule` with a static `RuleMeta` descriptor.

`RuleMeta` is the static contract used to filter and order rules, invalidate them after runtime knowledge mutations, schedule fixed-point attempts, and check eliminated forms after the full pipeline. `TransformContext::from_build_config` compiles that metadata into a `Plan`; `scheduler::drive_fixed_point` then runs the plan until no rule applies or `max_iterations` is exceeded.

See [`src/rewrite/rules/README.md`](src/rewrite/rules/README.md) for the authoritative metadata contract, including `triggers`, `eliminates`, `touches`, `produces`, dependency ordering, mutation filtering, convergence requirements, and the macro-based rule DSL.

### FinalizeAst

An idempotent pass (`src/finalize_ast/`) for profile-neutral AST representation canonicalization. Rewrite owns surface-to-semantic rewrites (for example `Command("prime") → Prime { count: 1 }`); FinalizeAst owns canonical AST shape: merging adjacent `Prime` nodes, merging adjacent text-mode `Text` siblings, collapsing ordinary lexer whitespace runs to one U+0020 without trimming edge spaces, and cleaning empty text (sequence children are deleted; empty `TextContent` slots become an empty implicit text group). The engine runs the same `finalize_ast::run` before FlattenGroups and again after FlattenGroups when that phase is enabled. The phase is enabled by default in every profile and gated by `TransformConfig.finalize_ast.enabled`.

### FlattenGroups

A single recursive traversal (`visit` → `try_unwrap` in `src/flatten_groups/mod.rs`). For each node the visitor:

1. Collects subtree-wide flags (`has_declarative`, `has_infix`, `has_delimited`) on the way down.
2. Tracks the `in_env_body` context flag through `Slot::EnvBody` edges.
3. On the way back up, calls `try_unwrap` to check whether the current group should be flattened. Each preserve guard short-circuits with an early return that increments its hit counter; the first matching guard wins.
4. If no guard fires and the group's content mode matches its parent's context mode, the group is unwrapped via either `unwrap_group_child` (multi-child splice) or `redirect_single_child_slot` (single-child slot replacement).

The `slot_can_unwrap` helper restricts redirect-style unwrapping to single-child groups in `Argument`, `Script*`, and `Infix*` slots; `EnvBody` slots are never unwrapped.

## Errors

Build errors reject invalid rewrite plans; run errors report rule failures, exhausted iteration limits, or residual eliminated forms. See [engine errors](src/error.rs) and [rewrite errors](src/rewrite/mod.rs) for variants. Errors must propagate through the facade and bindings rather than becoming successful reports.

## Tests

Keep individual rule cases inline and phase/interaction tests under `tests/`. Run `cargo test -p texform-transform` for the subsystem. Follow [TESTING.md](../../TESTING.md) for facade coverage and the required full corpus contract check.

## See also

- High-level overview: [repository README](../../README.md).
- Architecture: [`ARCHITECTURE.md`](../../ARCHITECTURE.md) (Transform Engine section).
- Rule authoring guide: [`src/rewrite/rules/README.md`](src/rewrite/rules/README.md).
