//! Transform engine that applies configured phases to an AST.
//!
//! The engine executes in this order:
//!
//! 1. **LowerAttributes** rewrites registered declarative-scope commands before
//!    ordinary rewrite execution.
//! 2. **Rewrite** runs transform rules in a fixed-point loop until the AST
//!    stabilizes (no rule fires) or the iteration limit is reached.
//! 3. **LowerAttributes** normalizes attribute prefixes created by Rewrite.
//! 4. **FinalizeAst** performs profile-neutral AST canonicalization (adjacent
//!    `Prime` merges, text-sequence normalization).
//! 5. **FlattenGroups** removes redundant grouping once earlier phases are complete.
//! 6. **FinalizeAst** again when FlattenGroups ran, so adjacency exposed by
//!    flattening is canonicalized. FinalizeAst is the last phase that mutates
//!    the AST.
//!
//! When Rewrite is enabled, after these steps the engine validates the resulting
//! AST against the eliminated-form contract derived into [`TransformContext`].
//! That validation is read-only.

use crate::ast::Ast;
use crate::config::TransformConfig;
use crate::context::TransformContext;
use crate::error::TransformError;
use crate::lower_attributes::LowerAttributesConfig;
use crate::parse::ParseContext;
use crate::report::TransformReport;
use crate::{finalize_ast, flatten_groups, lower_attributes, rewrite};

pub(crate) fn execute(
    tctx: &TransformContext,
    ast: &mut Ast,
    parse_ctx: &ParseContext,
    cfg: &TransformConfig,
) -> Result<TransformReport, TransformError> {
    let mut report = TransformReport::default();

    if cfg.lower_attributes_enabled {
        lower_attributes::run(
            ast,
            &LowerAttributesConfig::ENABLED,
            &mut report.lower_attributes,
        );
    }

    if cfg.rewrite_enabled {
        rewrite::run(
            ast,
            parse_ctx,
            tctx.rewrite_plan(),
            cfg.max_iterations,
            &mut report.rewrite,
        )
        .map_err(TransformError::Rewrite)?;
    }

    if cfg.lower_attributes_enabled {
        lower_attributes::run(
            ast,
            &LowerAttributesConfig::ENABLED,
            &mut report.lower_attributes,
        );
    }

    finalize_ast::run(ast, &cfg.finalize_ast, &mut report.finalize_ast);

    if cfg.flatten_groups.enabled {
        flatten_groups::run(ast, &cfg.flatten_groups, &mut report.flatten_groups);
        // FlattenGroups can expose new adjacent Prime / Text nodes. Re-run the
        // same idempotent FinalizeAst pass so sequence canonicalization is the
        // last AST mutation. Skip when FlattenGroups is off: the first pass
        // already finished the mutation pipeline for that input.
        finalize_ast::run(ast, &cfg.finalize_ast, &mut report.finalize_ast);
    }

    if cfg.rewrite_enabled
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

    Ok(report)
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
                rewrite_enabled: true,
                lower_attributes_enabled: true,
                finalize_ast: crate::FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::DISABLED,
                max_iterations: 100,
            },
            plan,
        );

        let report = context.run(&mut ast, &parse_ctx).expect(
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
