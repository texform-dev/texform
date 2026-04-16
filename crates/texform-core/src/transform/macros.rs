//! Authoring macros for builtin transform rules.
//!
//! External rule authors only interact with two entry points:
//! [`define_rule!`] for general rules and [`alias_rule!`] for simple prefix
//! command canonicalization. The `@with_meta_init`, `@impl_bind`, and `@impl`
//! arms below are internal dispatch details used to keep the user-facing forms
//! small without duplicating expansion logic.

macro_rules! define_rule {
    (
        $(#[$attr:meta])*
        pub static $static_name:ident : $rule_ty:ident {
            key: $group:ident / $name:literal,
            summary: $summary:expr,
            phase: $phase:ident,
            safety: $safety:ident,
            triggers: $triggers:expr,
            consumes: $consumes:expr,
            produces: $produces:expr,
            apply($rule:ident, $cx:ident, $node_id:ident) $body:block
        }
    ) => {
        $crate::transform::define_rule!(
            @impl_bind
            $($attr)*
            ;
            $static_name,
            $rule_ty,
            $group,
            $name,
            $summary,
            $phase,
            $safety,
            $triggers,
            $consumes,
            $produces,
            {},
            $rule,
            ($cx, $node_id),
            $body
        );
    };
    (
        $(#[$attr:meta])*
        pub static $static_name:ident : $rule_ty:ident {
            key: $group:ident / $name:literal,
            summary: $summary:expr,
            phase: $phase:ident,
            safety: $safety:ident,
            triggers: $triggers:expr,
            consumes: $consumes:expr,
            produces: $produces:expr,
            apply_fn: $apply_fn:path
        }
    ) => {
        $crate::transform::define_rule!(
            @impl
            [$($attr,)*],
            $static_name,
            $rule_ty,
            $group,
            $name,
            $summary,
            $phase,
            $safety,
            $triggers,
            $consumes,
            $produces,
            {},
            @apply_fn $apply_fn
        );
    };
    (
        @with_meta_init
        $(#[$attr:meta])*
        pub static $static_name:ident : $rule_ty:ident {
            key: $group:ident / $name:literal,
            summary: $summary:expr,
            phase: $phase:ident,
            safety: $safety:ident,
            triggers: $triggers:expr,
            consumes: $consumes:expr,
            produces: $produces:expr,
            meta_init $meta_init:block,
            apply($rule:ident, $cx:ident, $node_id:ident) $body:block
        }
    ) => {
        $crate::transform::define_rule!(
            @impl_bind
            $($attr)*
            ;
            $static_name,
            $rule_ty,
            $group,
            $name,
            $summary,
            $phase,
            $safety,
            $triggers,
            $consumes,
            $produces,
            $meta_init,
            $rule,
            ($cx, $node_id),
            $body
        );
    };
    (
        @with_meta_init
        $(#[$attr:meta])*
        pub static $static_name:ident : $rule_ty:ident {
            key: $group:ident / $name:literal,
            summary: $summary:expr,
            phase: $phase:ident,
            safety: $safety:ident,
            triggers: $triggers:expr,
            consumes: $consumes:expr,
            produces: $produces:expr,
            meta_init $meta_init:block,
            apply_fn: $apply_fn:path
        }
    ) => {
        $crate::transform::define_rule!(
            @impl
            [$($attr,)*],
            $static_name,
            $rule_ty,
            $group,
            $name,
            $summary,
            $phase,
            $safety,
            $triggers,
            $consumes,
            $produces,
            $meta_init,
            @apply_fn $apply_fn
        );
    };
    (
        @impl_bind
        $($attr:meta)*
        ;
        $static_name:ident,
        $rule_ty:ident,
        $group:ident,
        $name:literal,
        $summary:expr,
        $phase:ident,
        $safety:ident,
        $triggers:expr,
        $consumes:expr,
        $produces:expr,
        $meta_init:block,
        $rule:ident,
        ($cx:ident, $node_id:ident),
        $body:block
    ) => {
        $(#[$attr])*
        pub struct $rule_ty;

        pub static $static_name: $rule_ty = $rule_ty;

        impl $crate::transform::TransformRule for $rule_ty {
            fn meta(&self) -> &'static $crate::transform::RuleMeta {
                $meta_init
                static META: $crate::transform::RuleMeta = $crate::transform::RuleMeta {
                    key: $crate::transform::RuleKey {
                        group: $crate::transform::RuleGroup::$group,
                        name: $name,
                    },
                    summary: $summary,
                    phase: $crate::transform::RulePhase::$phase,
                    safety: $crate::transform::RuleSafety::$safety,
                    triggers: $triggers,
                    consumes: $consumes,
                    produces: $produces,
                };
                &META
            }

            fn apply(
                &self,
                cx: &mut $crate::transform::rule_context::RuleContext<'_>,
                node_id: $crate::ast::NodeId,
            ) -> Result<$crate::transform::RuleEffect, $crate::transform::TransformError> {
                let $rule = self;
                let $cx = cx;
                let $node_id = node_id;
                $body
            }
        }
    };
    (
        @impl
        [$($attr:meta,)*],
        $static_name:ident,
        $rule_ty:ident,
        $group:ident,
        $name:literal,
        $summary:expr,
        $phase:ident,
        $safety:ident,
        $triggers:expr,
        $consumes:expr,
        $produces:expr,
        $meta_init:block,
        @apply_fn $apply_fn:path
    ) => {
        $(#[$attr])*
        pub struct $rule_ty;

        pub static $static_name: $rule_ty = $rule_ty;

        impl $crate::transform::TransformRule for $rule_ty {
            fn meta(&self) -> &'static $crate::transform::RuleMeta {
                $meta_init
                static META: $crate::transform::RuleMeta = $crate::transform::RuleMeta {
                    key: $crate::transform::RuleKey {
                        group: $crate::transform::RuleGroup::$group,
                        name: $name,
                    },
                    summary: $summary,
                    phase: $crate::transform::RulePhase::$phase,
                    safety: $crate::transform::RuleSafety::$safety,
                    triggers: $triggers,
                    consumes: $consumes,
                    produces: $produces,
                };
                &META
            }

            fn apply(
                &self,
                cx: &mut $crate::transform::rule_context::RuleContext<'_>,
                node_id: $crate::ast::NodeId,
            ) -> Result<$crate::transform::RuleEffect, $crate::transform::TransformError> {
                $apply_fn(self, cx, node_id)
            }
        }
    };
}

macro_rules! cmd_targets {
    () => {
        &[] as &[$crate::transform::RuleTarget]
    };
    ($($record:expr),+ $(,)?) => {
        &[$($crate::transform::RuleTarget::Command($record)),+]
    };
}

#[allow(unused_macros)]
macro_rules! env_targets {
    () => {
        &[] as &[$crate::transform::RuleTarget]
    };
    ($($record:expr),+ $(,)?) => {
        &[$($crate::transform::RuleTarget::Environment($record)),+]
    };
}

macro_rules! cmd_triggers {
    () => {
        &[] as &[$crate::transform::RuleTrigger]
    };
    ($($record:expr),+ $(,)?) => {
        &[$($crate::transform::RuleTrigger::Command($record)),+]
    };
}

#[allow(unused_macros)]
macro_rules! env_triggers {
    () => {
        &[] as &[$crate::transform::RuleTrigger]
    };
    ($($record:expr),+ $(,)?) => {
        &[$($crate::transform::RuleTrigger::Environment($record)),+]
    };
}

macro_rules! alias_rule {
    (
        $(#[$attr:meta])*
        pub static $static_name:ident : $rule_ty:ident {
            key: $group:ident / $name:literal,
            summary: $summary:expr,
            phase: $phase:ident,
            safety: $safety:ident,
            canonical: $canonical:expr,
            aliases: [$($alias:expr),+ $(,)?],
        }
    ) => {
        $crate::transform::define_rule!(
            @with_meta_init
            $(#[$attr])*
            pub static $static_name: $rule_ty {
                key: $group / $name,
                summary: $summary,
                phase: $phase,
                safety: $safety,
                triggers: &[$($crate::transform::RuleTrigger::Command($alias)),+],
                consumes: $crate::transform::RuleConsumes {
                    eliminates: &[$($crate::transform::RuleTarget::Command($alias)),+],
                    requires: &[],
                },
                produces: $crate::transform::RuleProduces {
                    targets: &[$crate::transform::RuleTarget::Command($canonical)],
                },
                meta_init {
                    static ONCE: ::std::sync::Once = ::std::sync::Once::new();
                    ONCE.call_once(|| {
                        $crate::transform::macro_support::debug_assert_prefix_alias_group_compatible(
                            $canonical,
                            &[$($alias),+],
                        );
                    });
                },
                apply(_rule, cx, node_id) {
                    $crate::transform::macro_support::rename_prefix_command_alias(
                        cx,
                        node_id,
                        $canonical,
                        &[$(($alias).name),+],
                    )
                }
            }
        );
    };
}

pub(crate) use alias_rule;
pub(crate) use cmd_targets;
pub(crate) use cmd_triggers;
pub(crate) use define_rule;
pub(crate) use env_targets;
pub(crate) use env_triggers;

#[cfg(test)]
mod tests {
    use texform_specs::builtin::{ams, base};

    use crate::transform::{RuleTarget, RuleTrigger};

    #[test]
    fn cmd_targets_expands_to_command_target_slice() {
        assert_eq!(
            cmd_targets![&base::cmd::FRAC, &ams::cmd::FRAC],
            &[
                RuleTarget::Command(&base::cmd::FRAC),
                RuleTarget::Command(&ams::cmd::FRAC),
            ]
        );
    }

    #[test]
    fn env_targets_expands_to_environment_target_slice() {
        assert_eq!(
            env_targets![&ams::env::ALIGN],
            &[RuleTarget::Environment(&ams::env::ALIGN)]
        );
    }

    #[test]
    fn cmd_triggers_expands_to_command_trigger_slice() {
        assert_eq!(
            cmd_triggers![&base::cmd::OVER],
            &[RuleTrigger::Command(&base::cmd::OVER)]
        );
    }

    #[test]
    fn env_triggers_expands_to_environment_trigger_slice() {
        assert_eq!(
            env_triggers![&ams::env::ALIGN],
            &[RuleTrigger::Environment(&ams::env::ALIGN)]
        );
    }
}
