use texform_core::target_counter::{TargetCounter, count_node};

pub fn count_targets(
    parser: &crate::Parser,
    src: &str,
) -> Result<serde_json::Value, crate::ParseError> {
    count_targets_from_output(parser.parse(src))
}

pub fn count_targets_with(
    parser: &crate::Parser,
    src: &str,
    config: &texform_core::parse::ParseConfig,
) -> Result<serde_json::Value, crate::ParseError> {
    count_targets_from_output(parser.parse_with(src, config))
}

fn count_targets_from_output(
    output: crate::ParseResult,
) -> Result<serde_json::Value, crate::ParseError> {
    let (document, _) = output.try_into_document()?;
    let mut counter = TargetCounter::default();
    count_node(&document.to_syntax(), &mut counter);
    Ok(counter
        .logical_counts()
        .into_iter()
        .map(|(key, count)| (key, serde_json::Value::from(count)))
        .collect::<serde_json::Map<_, _>>()
        .into())
}
