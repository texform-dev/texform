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
            key: $package:ident / $name:literal,
            class: $class:ident,
            summary: $summary:expr,
            safety: $safety:ident,
            enabled_by_packages: [$($enabled_package:ident),+ $(,)?],
            triggers: $triggers:expr,
            consumes: $consumes:expr,
            produces: $produces:expr,
            apply($rule:ident, $cx:ident, $node_id:ident) $body:block
        }
    ) => {
        $crate::rewrite::define_rule!(
            @impl_bind
            $($attr)*
            ;
            $static_name,
            $rule_ty,
            $package,
            $name,
            $class,
            $summary,
            $safety,
            [$($enabled_package),+],
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
            key: $package:ident / $name:literal,
            class: $class:ident,
            summary: $summary:expr,
            safety: $safety:ident,
            enabled_by_packages: [$($enabled_package:ident),+ $(,)?],
            triggers: $triggers:expr,
            consumes: $consumes:expr,
            produces: $produces:expr,
            apply_fn: $apply_fn:path
        }
    ) => {
        $crate::rewrite::define_rule!(
            @impl
            [$($attr,)*],
            $static_name,
            $rule_ty,
            $package,
            $name,
            $class,
            $summary,
            $safety,
            [$($enabled_package),+],
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
            key: $package:ident / $name:literal,
            class: $class:ident,
            summary: $summary:expr,
            safety: $safety:ident,
            enabled_by_packages: [$($enabled_package:ident),+ $(,)?],
            triggers: $triggers:expr,
            consumes: $consumes:expr,
            produces: $produces:expr,
            meta_init $meta_init:block,
            apply($rule:ident, $cx:ident, $node_id:ident) $body:block
        }
    ) => {
        $crate::rewrite::define_rule!(
            @impl_bind
            $($attr)*
            ;
            $static_name,
            $rule_ty,
            $package,
            $name,
            $class,
            $summary,
            $safety,
            [$($enabled_package),+],
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
            key: $package:ident / $name:literal,
            class: $class:ident,
            summary: $summary:expr,
            safety: $safety:ident,
            enabled_by_packages: [$($enabled_package:ident),+ $(,)?],
            triggers: $triggers:expr,
            consumes: $consumes:expr,
            produces: $produces:expr,
            meta_init $meta_init:block,
            apply_fn: $apply_fn:path
        }
    ) => {
        $crate::rewrite::define_rule!(
            @impl
            [$($attr,)*],
            $static_name,
            $rule_ty,
            $package,
            $name,
            $class,
            $summary,
            $safety,
            [$($enabled_package),+],
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
        $package:ident,
        $name:literal,
        $class:ident,
        $summary:expr,
        $safety:ident,
        [$($enabled_package:ident),+],
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

        impl $rule_ty {
            pub const KEY: $crate::rewrite::RuleKey = $crate::rewrite::RuleKey {
                package: $crate::rewrite::PackageName::$package,
                name: $name,
            };
        }

        impl $crate::rewrite::RewriteRule for $rule_ty {
            fn meta(&self) -> &'static $crate::rewrite::RuleMeta {
                $meta_init
                static META: $crate::rewrite::RuleMeta = $crate::rewrite::RuleMeta {
                    key: $rule_ty::KEY,
                    enabled_by_packages: &[
                        $($crate::rewrite::PackageName::$enabled_package),+
                    ],
                    class: $crate::rewrite::RuleClass::$class,
                    summary: $summary,
                    safety: $crate::rewrite::RuleSafety::$safety,
                    triggers: $triggers,
                    consumes: $consumes,
                    produces: $produces,
                };
                &META
            }

            fn apply(
                &self,
                cx: &mut $crate::rewrite::rule_context::RuleContext<'_>,
                node_id: $crate::ast::NodeId,
            ) -> Result<$crate::rewrite::RuleEffect, $crate::rewrite::RuleError> {
                let $rule = self;
                let _ = $rule;
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
        $package:ident,
        $name:literal,
        $class:ident,
        $summary:expr,
        $safety:ident,
        [$($enabled_package:ident),+],
        $triggers:expr,
        $consumes:expr,
        $produces:expr,
        $meta_init:block,
        @apply_fn $apply_fn:path
    ) => {
        $(#[$attr])*
        pub struct $rule_ty;

        pub static $static_name: $rule_ty = $rule_ty;

        impl $rule_ty {
            pub const KEY: $crate::rewrite::RuleKey = $crate::rewrite::RuleKey {
                package: $crate::rewrite::PackageName::$package,
                name: $name,
            };
        }

        impl $crate::rewrite::RewriteRule for $rule_ty {
            fn meta(&self) -> &'static $crate::rewrite::RuleMeta {
                $meta_init
                static META: $crate::rewrite::RuleMeta = $crate::rewrite::RuleMeta {
                    key: $rule_ty::KEY,
                    enabled_by_packages: &[
                        $($crate::rewrite::PackageName::$enabled_package),+
                    ],
                    class: $crate::rewrite::RuleClass::$class,
                    summary: $summary,
                    safety: $crate::rewrite::RuleSafety::$safety,
                    triggers: $triggers,
                    consumes: $consumes,
                    produces: $produces,
                };
                &META
            }

            fn apply(
                &self,
                cx: &mut $crate::rewrite::rule_context::RuleContext<'_>,
                node_id: $crate::ast::NodeId,
            ) -> Result<$crate::rewrite::RuleEffect, $crate::rewrite::RuleError> {
                $apply_fn(self, cx, node_id)
            }
        }
    };
}

macro_rules! cmd_targets {
    () => {
        &[] as &[$crate::rewrite::RuleTarget]
    };
    ($($record:expr),+ $(,)?) => {
        &[$($crate::rewrite::RuleTarget::Command($record)),+]
    };
}

#[allow(unused_macros)]
macro_rules! env_targets {
    () => {
        &[] as &[$crate::rewrite::RuleTarget]
    };
    ($($record:expr),+ $(,)?) => {
        &[$($crate::rewrite::RuleTarget::Environment($record)),+]
    };
}

#[allow(unused_macros)]
macro_rules! char_targets {
    () => {
        &[] as &[$crate::rewrite::RuleTarget]
    };
    ($($record:expr),+ $(,)?) => {
        &[$($crate::rewrite::RuleTarget::Character($record)),+]
    };
}

macro_rules! alias_rule {
    (
        $(#[$attr:meta])*
        pub static $static_name:ident : $rule_ty:ident {
            key: $package:ident / $name:literal,
            class: $class:ident,
            summary: $summary:expr,
            safety: $safety:ident,
            enabled_by_packages: [$($enabled_package:ident),+ $(,)?],
            canonical: $canonical:expr,
            aliases: [$($alias:expr),+ $(,)?],
        }
    ) => {
        $crate::rewrite::define_rule!(
            @with_meta_init
            $(#[$attr])*
            pub static $static_name: $rule_ty {
                key: $package / $name,
                class: $class,
                summary: $summary,
                safety: $safety,
                enabled_by_packages: [$($enabled_package),+],
                triggers: &[$($crate::rewrite::RuleTarget::Command($alias)),+],
                consumes: $crate::rewrite::RuleConsumes {
                    eliminates: &[$($crate::rewrite::RuleTarget::Command($alias)),+],
                    touches: &[],
                },
                produces: $crate::rewrite::RuleProduces {
                    targets: &[$crate::rewrite::RuleTarget::Command($canonical)],
                },
                meta_init {
                    static ONCE: ::std::sync::Once = ::std::sync::Once::new();
                    ONCE.call_once(|| {
                        $crate::rewrite::macro_support::debug_assert_prefix_alias_package_compatible(
                            $canonical,
                            &[$($alias),+],
                        );
                    });
                },
                apply(_rule, cx, node_id) {
                    $crate::rewrite::macro_support::rename_prefix_command_alias(
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

#[cfg(test)]
macro_rules! transform_examples {
    (
        rule: $rule:expr,
        class: $class:ident,
        examples: [
            $({
                label: $label:ident,
                packages: [$($pkg:literal),+ $(,)?],
                input: $input:expr,
                expected: $expected:expr $(,)?
            }),+ $(,)?
        ]
    ) => {
        $(
            #[test]
            fn $label() {
                use $crate::rewrite::RewriteRule as _;
                let parse_ctx = $crate::parse::ParseContext::from_packages(&[$($pkg),+]);
                let build_config = $crate::BuildConfig::profile($crate::Profile::Authoring)
                    .rewrite_classes($crate::RuleClassSet::from($crate::RuleClass::$class))
                    .only_rule_for_tests($rule.meta().key);
                let transform_context = $crate::TransformContext::from_build_config(
                    build_config,
                    &parse_ctx,
                )
                .expect("transform context should build");
                let cfg = $crate::TransformConfig {
                    rewrite_enabled: true,
                    lower_attributes_enabled: false,
                    flatten_groups: $crate::FlattenGroupsConfig::DISABLED,
                    max_iterations: 100,
                };
                let parse_config = $crate::parse::ParseConfig::STRICT;

                let mut ast = parse_ctx
                    .parse_to_ast($input, &parse_config)
                    .expect("parse input should succeed");
                transform_context.run_with(&mut ast, &parse_ctx, &cfg)
                    .expect("transform should succeed");
                let actual = $crate::serialize::serialize(&ast);

                let expected_ast = parse_ctx
                    .parse_to_ast($expected, &parse_config)
                    .expect("parse expected should succeed");
                let canonical_expected = $crate::serialize::serialize(&expected_ast);

                assert_eq!(actual, canonical_expected,
                    "transform output differs from expected (both serialized)\n  input:    {}\n  actual:   {}\n  expected: {}",
                    $input, actual, canonical_expected);
            }
        )+
    };
}

pub(crate) use alias_rule;
pub(crate) use char_targets;
pub(crate) use cmd_targets;
pub(crate) use define_rule;
pub(crate) use env_targets;
#[cfg(test)]
pub(crate) use transform_examples;

#[cfg(test)]
mod tests {
    use texform_knowledge::builtin::{ams, base, bboldx};

    use crate::rewrite::{RuleTarget, RuleTargetKind};

    #[test]
    fn cmd_targets_expands_to_command_target_slice() {
        assert_eq!(
            cmd_targets![&base::cmd::FRAC],
            &[RuleTarget::Command(&base::cmd::FRAC)]
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
    fn char_targets_wraps_character_records() {
        let targets = char_targets![&bboldx::chars::BBDOTLESSI];
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].key().kind, RuleTargetKind::Character);
        assert_eq!(targets[0].key().name, "bbdotlessi");
    }
}
