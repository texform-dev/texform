//! Canonicalize trace aliases into the lowercase `\tr` command.

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Canonicalize `\Tr`, `\trace`, and `\Trace` into `\tr`.
    pub static TRACE_TO_TR: TraceToTrRule {
        key: Physics / "trace-to-tr",
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

#[cfg(test)]
mod tests {
    use crate::ast::Node;
    use crate::parse::ParseContext;
    use crate::transform::{transform_ast, BuiltinRuleSetId, TransformContextBuilder};

    #[test]
    fn rewrites_all_trace_aliases_to_tr() {
        let parse_ctx = ParseContext::from_packages(&["physics"]);
        let transform_ctx = TransformContextBuilder::new(BuiltinRuleSetId::Normalize)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        for input in [r"\Tr", r"\trace", r"\Trace"] {
            let mut ast = parse_ctx
                .parse_to_ast(input, true)
                .expect("parse should succeed")
                .ast;
            let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
                .unwrap_or_else(|error| panic!("trace-to-tr transform should succeed: {error:?}"));

            assert_eq!(output.applied.len(), 1);
            assert_eq!(output.applied[0].key.to_string(), "physics/trace-to-tr");

            let root = ast.root();
            let children = ast.children(root);
            assert_eq!(children.len(), 1);

            match ast.node(children[0]) {
                Node::Command { name, args, .. } => {
                    assert_eq!(name, "tr");
                    assert!(args.is_empty());
                }
                other => panic!("expected tr command after transform, got {:?}", other),
            }
        }
    }
}
