use texform_core::target_counter::{TargetCounter, count_node};

pub fn count_targets(
    parser: &crate::Parser,
    src: &str,
) -> Result<serde_json::Value, texform_core::parse::ParseAstError> {
    count_targets_from_output(parser.parse(src))
}

pub fn count_targets_with(
    parser: &crate::Parser,
    src: &str,
    config: &texform_core::parse::ParseConfig,
) -> Result<serde_json::Value, texform_core::parse::ParseAstError> {
    count_targets_from_output(parser.parse_with(src, config))
}

fn count_targets_from_output(
    output: texform_core::parse::ParseOutput,
) -> Result<serde_json::Value, texform_core::parse::ParseAstError> {
    match (output.result, output.diagnostics) {
        (Some(result), diagnostics) if diagnostics.is_empty() => {
            let mut counter = TargetCounter::default();
            count_node(&result.node, &mut counter);
            Ok(serde_json::to_value(counter.logical_counts())
                .expect("target counts should serialize"))
        }
        (Some(_), diagnostics) => {
            Err(texform_core::parse::ParseAstError::DiagnosticsPresent { diagnostics })
        }
        (None, diagnostics) => {
            Err(texform_core::parse::ParseAstError::NoParseResult { diagnostics })
        }
    }
}
