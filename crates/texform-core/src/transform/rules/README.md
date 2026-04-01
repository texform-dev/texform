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
