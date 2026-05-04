# Transform Rules

This directory stores concrete transform rules.

## Adding a New Rule

1. Create a new `.rs` file under the package/tier/group layout, for example
   `base/base/over-family/over_to_frac.rs` or
   `physics/base/trace-alias/trace_to_tr.rs`.
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

Rules use this directory structure:

```text
<package>/<tier>/<directory_group>/<rule_file_stem>.rs
```

- `package` is the owning rule package, such as `base`, `ams`, or `physics`.
- `tier` is the profile tier: `base`, `expand`, or `deep`.
- `directory_group` is only a human-readable grouping slug. It is not part of
  `RuleMeta` and does not affect scheduling.

The rule key remains `<package>/<name>`, independent of the directory group.

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
use crate::transform::{alias_rule, cmd_targets, define_rule, env_targets};
```

These macros are intentionally local to `texform-core`; they are ergonomics
helpers for builtin rules, not a public rule-definition API.

## Builtin Record Imports

Always import builtin records through an explicit package module:

```rust
use texform_specs::builtin::base;
use texform_specs::builtin::ams;
```

When referencing builtin records in consumes or produces, always use
the package-qualified path:

```rust
RuleTarget::Command(&base::cmd::FRAC)
RuleTarget::Environment(&ams::env::ALIGN)
```

The target contract is package-insensitive: each target means `kind + name`.
The package-qualified Rust path exists because `RuleTarget` stores a concrete
builtin record reference. For each `kind + name`, choose the first package that
defines that record in texform package import order.

## Package Variants

Do not duplicate same-name package variants in rule metadata. `RuleConsumes`
and `RuleProduces` are interpreted as `kind + name`, so each target appears once:

```rust
use texform_specs::builtin::base;

consumes: RuleConsumes {
    eliminates: cmd_targets![&base::cmd::OVER],
    touches: &[],
},
produces: RuleProduces {
    targets: cmd_targets![&base::cmd::FRAC],
},
```

If the same command or environment name exists in multiple packages, choose the
first builtin record by texform package import order. `enabled_by_packages`
declares which input packages make the rule loadable; it does not constrain
which package supplies a produced target.

Package-specific split decisions are based on structural signatures:

1. Commands use `CommandKind + argspec.source`
2. Environments use `argspec.source + body_mode`
3. Same signature means one rule with all matching packages in
   `enabled_by_packages`
4. Different signatures mean separate rules

The transform plan collapses every target to `RuleTargetKey` (`kind + name`) for
topological sort, cleanup-boundary checks, mutation filtering, and
eliminated-form derivation.

## define_rule!

Use `define_rule!` when the rule metadata is regular but the AST rewrite logic
still needs ordinary Rust code:

```rust
define_rule! {
    pub static OVER_TO_FRAC: OverToFracRule {
        key: Base / "over-to-frac",
        tier: Base,
        summary: "Rewrite infix \\over into prefix \\frac",
        phase: Normalize,
        safety: Semantic,
        enabled_by_packages: [Base],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::OVER],
            touches: &[],
        },
        produces: RuleProduces {
            targets: cmd_targets![&base::cmd::FRAC],
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
        key: Physics / "trace-to-tr",
        tier: Base,
        summary: "Canonicalize \\Tr, \\trace, and \\Trace into \\tr",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
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

`alias_rule!` declares aliases as eliminated commands and the canonical command
as the produced command. The engine attempts the rule when the current node
matches one of the alias command names.

Do not use `alias_rule!` for:

1. Package-variant handling of same-name commands
2. Character-backed commands such as base `\Re`
3. Infix, declarative, or environment canonicalization
4. Rules that need any AST surgery beyond renaming a prefix command

## Sugar Macros

Use the small metadata helpers when they reduce noise:

```rust
cmd_targets![&base::cmd::FRAC]
env_targets![&ams::env::ALIGN]
```

These macros only wrap builtin paths into `RuleTarget::*` arrays. They do not
infer package variants, enabled packages, canonical forms, or any other rule
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

## Transform Profiles

Use `TransformProfile` to build contexts in tests and examples:

```rust
let transform_ctx = TransformProfile::AUTHORING
    .builder()
    .only(OVER_TO_FRAC.meta().key)
    .build_with(&parse_ctx)?;
```

Profiles select rules by tier before `only` or `disable` is applied:

- `AUTHORING` includes `RuleTier::Base`
- `CORPUS` includes `RuleTier::Base` and `RuleTier::Expand`
- `EQUIV` includes all tiers

`only` is an allowlist inside the selected profile; it does not enable an
`expand` or `deep` rule in a narrower profile.
