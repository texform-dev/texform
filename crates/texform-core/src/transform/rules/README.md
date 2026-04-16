# Transform Rules

This directory stores concrete transform rules.

## Adding a New Rule

1. Create a new `.rs` file anywhere under this directory tree (e.g., `my_rule.rs`
   or `desugar/my_rule.rs`).
2. Define and export exactly one `pub static MY_RULE: MyRuleType` where the
   constant name is the UPPER_SNAKE_CASE form of the file stem.
3. That's it — the build script auto-discovers the file and registers it.

No manual edits to `mod.rs` are required. The `build.rs` at the crate root
scans this directory at compile time, generates absolute `#[path]` module
declarations, and aggregates every rule constant into `ALL_RULES`.

## File Layout

Each rule must live in a single file.

Keep the following pieces together in that file:

1. The rule type itself
2. Small rule-local helpers
3. Inline tests under `#[cfg(test)] mod tests`

`mod.rs` is generated — it only contains `include!(…)` pointing at the
build-script output. Do not edit it by hand.

Prefer defining metadata as a function-local static inside `meta()`:

```rust
fn meta(&self) -> &'static RuleMeta {
    static META: RuleMeta = RuleMeta { /* ... */ };
    &META
}
```

This keeps the metadata physically close to the trait implementation without
adding another file-level symbol for every rule.

For repeated rule shells, prefer the crate-private authoring macros:

```rust
use crate::transform::{alias_rule, cmd_targets, cmd_triggers, define_rule};
```

These macros are intentionally local to `texform-core`; they are ergonomics
helpers for builtin rules, not a public rule-definition API.

## Builtin Record Imports

Always import builtin records through an explicit package module:

```rust
use texform_specs::builtin::base;
use texform_specs::builtin::ams;
```

When referencing builtin records in triggers, consumes, or produces, always use
the package-qualified path:

```rust
RuleTrigger::Command(&base::cmd::OVER)
RuleTarget::Command(&base::cmd::FRAC)
RuleTarget::Environment(&ams::env::ALIGN)
```

The package name must stay visible at use sites so the origin of the builtin
record is obvious during authoring and review.

## Package Variants

Some builtins exist in multiple packages with the same semantic shape. When
that happens, list every compatible package variant in `triggers`,
`consumes`, and `produces`:

```rust
use texform_specs::builtin::{ams, base};

triggers: &[
    RuleTrigger::Command(&base::cmd::FRAC),
    RuleTrigger::Command(&ams::cmd::FRAC),
],
produces: RuleProduces {
    targets: &[
        RuleTarget::Command(&base::cmd::FRAC),
        RuleTarget::Command(&ams::cmd::FRAC),
    ],
},
```

Authoring rules with package variants follows three constraints:

1. Only group variants that share the same structural shape.
   For commands this means identical `kind` and `argspec.source`.
   For environments this means identical `argspec.source` and `body_mode`.
2. If two packages define the same name with different specs, split them into
   separate rules instead of mixing incompatible variants in one metadata block.
3. `apply()` may keep using any one package-qualified record with
   `match_*` helpers or constructors such as `prefix_command`.
   Runtime matching is name-based, so the package of the chosen record does not
   affect behavior.

Package variants are collapsed by `RuleTargetKey` (`kind + name`) when the
transform plan is built and executed:

1. All same-name package variants inside one target group must stay
   structurally consistent. Mixing incompatible variants is an authoring bug.
2. Structurally consistent variants collapse to the same runtime dependency key,
   so grouping them does not change topo sort, cleanup-boundary checks, or
   eliminated-form derivation.

Trigger availability is intentionally not validated during transform-context
construction:

1. Triggers are only OR-matched by the engine at runtime.
2. Triggers do not participate in topo sort or eliminated-form derivation.
3. A missing trigger therefore degrades to a no-op instead of a build error.
   If a recognized form still remains after a rule promises to eliminate it,
   final eliminated-form validation catches the mismatch.

## define_rule!

Use `define_rule!` when the rule metadata is regular but the AST rewrite logic
still needs ordinary Rust code:

```rust
define_rule! {
    pub static OVER_TO_FRAC: OverToFracRule {
        key: Structural / "over-to-frac",
        summary: "Rewrite infix \\over into prefix \\frac",
        phase: Normalize,
        safety: Semantic,
        triggers: cmd_triggers![&base::cmd::OVER],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::OVER],
            requires: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::FRAC, &ams::cmd::FRAC],
        },
        apply(rule, cx, node_id) {
            // normal Rust body
        }
    }
}
```

Prefer this macro for rules that:

1. Rebuild nodes or subtrees
2. Need shape validation with `RuleContext`
3. Need bespoke matching logic beyond simple rename canonicalization

The inline form always binds an explicit rule variable, such as `rule`, so the
body can call `rule.meta().key` without relying on a magic `self` binding.

When IDE navigation matters more than keeping the body inline, use the
`apply_fn: path` variant and move the rewrite code into a normal function.

## alias_rule!

Use `alias_rule!` only for prefix-command canonicalization where aliases and
the canonical command share the same `allowed_mode` and `argspec.source`, and
the rule only renames the command:

```rust
alias_rule! {
    pub static TRACE_TO_TR: TraceToTrRule {
        key: Canonical / "trace-to-tr",
        summary: "Canonicalize \\Tr, \\trace, and \\Trace into \\tr",
        phase: Normalize,
        safety: Lossless,
        canonical: &physics::cmd::TR,
        aliases: [
            &physics::cmd::TR_2,
            &physics::cmd::TRACE,
            &physics::cmd::TRACE_2,
        ],
    }
}
```

`alias_rule!` enforces only structural invariants:

1. Canonical and alias commands must all be `Prefix`
2. `allowed_mode` must match
3. `argspec.source` must match
4. The alias list must be non-empty and must not contain the canonical command

`alias_rule!` does not validate tag equality. If aliases and the canonical
command carry different tags, downstream `CommandTag`-based behavior will follow
the canonical command after rename. That semantic choice belongs to the rule
author, not to the macro.

Do not use `alias_rule!` for:

1. Package-variant handling of same-name commands
2. Character-backed commands such as base `\Re`
3. Infix, declarative, or environment canonicalization
4. Rules that need any AST surgery beyond renaming a prefix command

## Sugar Macros

Use the small metadata helpers when they reduce noise:

```rust
cmd_targets![&base::cmd::FRAC, &ams::cmd::FRAC]
env_targets![&ams::env::ALIGN]
cmd_triggers![&base::cmd::OVER]
env_triggers![&ams::env::ALIGN]
```

These macros only wrap builtin paths into `RuleTarget::*` or `RuleTrigger::*`
arrays. They do not infer package variants, canonical forms, or any other rule
semantics.

## Shared Helper Imports

For shared transform helpers, import the specific functions you use:

```rust
use crate::transform::helpers::{mandatory_content, prefix_command};
```

Use `RuleContext` helpers for node matching and shape checks when possible:

```rust
let Some(infix) = cx.match_infix(node_id, &base::cmd::OVER) else {
    return Ok(RuleEffect::Skipped);
};
cx.expect_no_args(rule.meta().key, infix.args, "\\over")?;
```

Preferred style:

1. Keep package prefixes for builtin records, such as `base::cmd::OVER`
2. Import shared constructor helpers directly, such as `prefix_command` and `mandatory_content`
3. Prefer `RuleContext` match/shape helpers over open-coded `match` + repeated error construction
