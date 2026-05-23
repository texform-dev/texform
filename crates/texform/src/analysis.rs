use texform_core::target_counter::{TargetCounter, count_node};

pub fn count_targets(
    parser: &crate::Parser,
    src: &str,
    config: &texform_core::parse::ParseConfig,
) -> Result<serde_json::Value, texform_core::parse::ParseAstError> {
    let output = parser.parse_with(src, config);
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
