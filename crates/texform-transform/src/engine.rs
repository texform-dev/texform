//! Transform phases run in deterministic LowerAttributes → Rewrite →
//! LowerAttributes → FinalizeAst → FlattenGroups rounds. Each phase reaches
//! its own fixed point and reports mutations independently of diagnostics.
//! A phase is skipped when it has already processed the current AST version.
//! Any mutation invalidates the other phases. After at most eight rounds the
//! engine either reaches their common fixed point or returns NotConverged.
//! Eliminated-form validation runs once afterward and is read-only.

use crate::ast::Ast;
use crate::config::TransformConfig;
use crate::context::TransformContext;
use crate::error::TransformError;
use crate::flatten_groups::FlattenGroupsGuardsOverlay;
use crate::parse::ParseContext;
use crate::report::ReportRecorder;
use crate::{finalize_ast, flatten_groups, lower_attributes, rewrite};

pub(crate) fn execute(
    tctx: &TransformContext,
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    cfg: &TransformConfig,
    flatten_groups_overlay: Option<&FlattenGroupsGuardsOverlay>,
    recorder: &mut ReportRecorder,
) -> Result<(), TransformError> {
    const MAX_ROUNDS: usize = 8;
    let enabled = [
        cfg.lower_attributes.enabled,
        cfg.rewrite.enabled,
        cfg.finalize_ast.enabled,
        cfg.flatten_groups.enabled,
    ];
    let mut guards = crate::flatten_groups::FlattenGroupsGuards::from_config(cfg.flatten_groups);
    if let Some(overlay) = flatten_groups_overlay {
        guards.apply_overlay(*overlay);
    }
    let mut version = 0;
    let mut seen = [None; 4];
    let mut rounds = 0;
    while enabled
        .iter()
        .enumerate()
        .any(|(phase, enabled)| *enabled && seen[phase] != Some(version))
    {
        if rounds == MAX_ROUNDS {
            return Err(TransformError::NotConverged {
                max_rounds: MAX_ROUNDS,
            });
        }
        rounds += 1;
        // Each phase reaches its own fixed point. Any mutation invalidates all
        // other phases, including those earlier in this deterministic order.
        for phase in [0, 1, 0, 2, 3] {
            if !enabled[phase] || seen[phase] == Some(version) {
                continue;
            }
            let changed = match phase {
                0 => lower_attributes::run(ast, &cfg.lower_attributes, recorder),
                1 => rewrite::run(
                    ast,
                    parse_ctx,
                    tctx.rewrite_plan(),
                    cfg.rewrite.max_iterations,
                    recorder,
                )
                .map_err(TransformError::Rewrite)?,
                2 => finalize_ast::run(ast, &cfg.finalize_ast, recorder),
                3 => flatten_groups::run(ast, &guards, recorder),
                _ => unreachable!("phase index comes from the fixed schedule"),
            };
            if changed {
                version += 1;
            }
            seen[phase] = Some(version);
        }
    }

    if cfg.rewrite.enabled
        && let Some(violation) = rewrite::collect_eliminated_violations(
            ast,
            parse_ctx,
            tctx.rewrite_plan().eliminated_forms(),
        )
        .into_iter()
        .next()
    {
        return Err(TransformError::Rewrite(
            rewrite::RewriteError::ContractViolation {
                target: violation.target,
                node_name: violation.node_name,
            },
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use texform_knowledge::builtin::{base, physics};

    use super::*;
    use crate::ast::{Node, NodeId};
    use crate::flatten_groups::FlattenGroupsConfig;
    use crate::parse::{ParseConfig, ParseContext};
    use crate::rewrite::rule_context::RuleContext;
    use crate::rewrite::{
        PackageName, Plan as RewritePlan, RewriteRule, RuleConsumes, RuleEffect, RuleFidelity,
        RuleKey, RuleLevel, RuleMeta, RuleProduces, RuleTarget,
    };
    use crate::serialize::serialize;

    #[test]
    fn transform_contract_final_checkpoint_runs_after_post_lower_attributes() {
        let parse_ctx =
            ParseContext::from_packages(&["base", "textmacros", "physics", "boldsymbol"]);
        let mut ast = parse_to_ast(&parse_ctx, r"\vb{\rm x}");
        let plan = RewritePlan::from_rules_for_tests(vec![&VB_TO_MATHBF_FOR_CONTRACT_TEST]);
        let context = TransformContext::from_rewrite_plan_for_tests(
            TransformConfig {
                lower_attributes: crate::LowerAttributesConfig::ENABLED,
                rewrite: crate::RewriteConfig::DEFAULT,
                finalize_ast: crate::FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::DISABLED,
            },
            plan,
        );

        let report = context
            .run_with_report(&mut ast, &parse_ctx, context.default_config())
            .expect(
                "post LowerAttributes should clear the generated bold prefix before contract check",
            );

        ast.assert_invariants();
        assert_eq!(serialize(&ast), r"\mathrm { x }");
        assert_eq!(report.rewrite.rules[0].applied_count, 1);
        assert!(
            rewrite::collect_eliminated_violations(
                &ast,
                &parse_ctx,
                context.rewrite_plan().eliminated_forms(),
            )
            .is_empty()
        );
    }

    #[test]
    fn phase_feedback_cycle_returns_not_converged() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = parse_to_ast(&parse_ctx, r"\mathbf{x}");
        let context = TransformContext::from_rewrite_plan_for_tests(
            TransformConfig {
                lower_attributes: crate::LowerAttributesConfig::ENABLED,
                rewrite: crate::RewriteConfig::DEFAULT,
                finalize_ast: crate::FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRICT,
            },
            RewritePlan::from_rules_for_tests(vec![&WRAP_SINGLETON]),
        );
        let error = context.run(&mut ast, &parse_ctx).unwrap_err();
        assert_eq!(error, TransformError::NotConverged { max_rounds: 8 });
        ast.assert_invariants();
    }

    struct WrapSingleton;
    static WRAP_SINGLETON: WrapSingleton = WrapSingleton;
    impl RewriteRule for WrapSingleton {
        fn meta(&self) -> &'static RuleMeta {
            static META: RuleMeta = RuleMeta {
                key: RuleKey {
                    package: PackageName::Base,
                    name: "wrap-singleton-test",
                },
                enabled_by_packages: &[PackageName::Base],
                level: RuleLevel::Authoring,
                summary: "Recreate a singleton group removed by LowerAttributes.",
                fidelity: RuleFidelity::Render,
                triggers: &[RuleTarget::Command(&base::cmd::MATHBF)],
                consumes: RuleConsumes {
                    eliminates: &[],
                    touches: &[RuleTarget::Command(&base::cmd::MATHBF)],
                },
                produces: RuleProduces { targets: &[] },
            };
            &META
        }
        fn apply(
            &self,
            cx: &mut RuleContext<'_>,
            node: NodeId,
        ) -> Result<RuleEffect, rewrite::RuleError> {
            let Some(command) = cx.match_command(node, &base::cmd::MATHBF) else {
                return Ok(RuleEffect::Skipped);
            };
            let Some(crate::ast::ArgumentValue::MathContent(body)) =
                command.args[0].as_ref().map(|a| a.value.clone())
            else {
                return Ok(RuleEffect::Skipped);
            };
            if matches!(cx.ast.node(body), Node::Group { .. }) {
                return Ok(RuleEffect::Skipped);
            }
            // R itself converges; L removes this wrapper and reactivates R.
            let group = cx.ast.implicit_math_group(Vec::new());
            cx.ast.replace_content_child(body, group);
            cx.ast.append_child(group, body);
            Ok(RuleEffect::Applied)
        }
    }

    fn parse_to_ast(parse_ctx: &ParseContext, src: &str) -> Ast {
        let document = parse_ctx
            .parse(src, &ParseConfig::default())
            .try_into_document()
            .expect("source should parse")
            .0;
        Ast::from_syntax_root(&document.to_syntax())
    }

    struct VbToMathbfForContractTest;

    static VB_TO_MATHBF_FOR_CONTRACT_TEST: VbToMathbfForContractTest = VbToMathbfForContractTest;

    impl VbToMathbfForContractTest {
        const KEY: RuleKey = RuleKey {
            package: PackageName::Physics,
            name: "vb-to-mathbf-contract-test",
        };
    }

    impl RewriteRule for VbToMathbfForContractTest {
        fn meta(&self) -> &'static RuleMeta {
            static META: RuleMeta = RuleMeta {
                key: VbToMathbfForContractTest::KEY,
                enabled_by_packages: &[PackageName::Physics],
                level: RuleLevel::Faithful,
                summary: "Create a bold prefix that the post LowerAttributes pass removes.",
                fidelity: RuleFidelity::Render,
                triggers: &[RuleTarget::Command(&physics::cmd::VB)],
                consumes: RuleConsumes {
                    eliminates: &[RuleTarget::Command(&base::cmd::MATHBF)],
                    touches: &[RuleTarget::Command(&physics::cmd::VB)],
                },
                produces: RuleProduces {
                    targets: &[RuleTarget::Command(&base::cmd::MATHBF)],
                },
            };
            &META
        }

        fn apply(
            &self,
            cx: &mut RuleContext<'_>,
            node_id: NodeId,
        ) -> Result<RuleEffect, rewrite::RuleError> {
            let Some(command) = cx.match_command(node_id, &physics::cmd::VB) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY)
                .expect_arg_len(command.args, 2, r"\vb")?;
            let body = command.args[1].clone();

            cx.ast.replace_node(
                node_id,
                Node::Command {
                    name: base::cmd::MATHBF.name.to_string(),
                    args: vec![body],
                    known: true,
                },
            );

            Ok(RuleEffect::Applied)
        }
    }
}
