# Transform Rules

This directory stores concrete transform rules.

## File Layout

Each rule must live in a single file.

Keep the following pieces together in that file:

1. The rule type itself
2. Small rule-local helpers
3. Inline tests under `#[cfg(test)] mod tests`

`mod.rs` should only list available rule modules. Do not move rule behavior or
rule-specific tests into `mod.rs` or external integration test files unless
there is a strong reason.

Prefer defining metadata as a function-local static inside `meta()`:

```rust
fn meta(&self) -> &'static RuleMeta {
    static META: RuleMeta = RuleMeta { /* ... */ };
    &META
}
```

This keeps the metadata physically close to the trait implementation without
adding another file-level symbol for every rule.

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
   For commands this means identical `kind` and `spec_string`.
   For environments this means identical `spec_string` and `body_mode`.
2. If two packages define the same name with different specs, split them into
   separate rules instead of mixing incompatible variants in one metadata block.
3. `apply()` may keep using any one package-qualified record with
   `match_*` helpers or constructors such as `prefix_command`.
   Runtime matching is name-based, so the package of the chosen record does not
   affect behavior.

The compile step enforces one invariant in debug builds:

1. All same-name package variants inside one target group must stay
   structurally consistent. Violations are programming errors and trip a
   `debug_assert!`.
2. Structurally consistent variants are availability-equivalent for the same
   active record, so grouping them does not change single-variant semantics.

Trigger availability is intentionally not validated at compile time:

1. Triggers are only OR-matched by the engine at runtime.
2. Triggers do not participate in topo sort or normal-form contract derivation.
3. A missing trigger therefore degrades to a no-op instead of a compile error.
   If a rule never fires but still promises to eliminate a form, final contract
   validation catches the mismatch.

## Shared Helper Imports

For shared transform helpers, import the specific functions you use:

```rust
use crate::transform::helpers::{mandatory_content, prefix_command};
```

Use `TransformContext` helpers for node matching and shape checks when possible:

```rust
let Some(infix) = cx.match_infix(node_id, &base::cmd::OVER) else {
    return Ok(RuleEffect::Skipped);
};
cx.expect_no_args(self.meta().key, infix.args, "\\over")?;
```

Preferred style:

1. Keep package prefixes for builtin records, such as `base::cmd::OVER`
2. Import shared constructor helpers directly, such as `prefix_command` and `mandatory_content`
3. Prefer `TransformContext` match/shape helpers over open-coded `match` + repeated error construction
