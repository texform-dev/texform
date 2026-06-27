//! Corpus-analysis helpers built on the parser.
//!
//! These functions parse a formula and report aggregate structure rather than
//! returning an editable tree. They are the building blocks behind data-product
//! tooling that summarizes large LaTeX corpora.

use texform_core::target_counter::{TargetCounter, count_node};

/// Count normalization targets in a formula, using the parser's default config.
///
/// Parses `src` with `parser`, then walks the resulting tree counting the
/// logical occurrences of each normalization target (the constructs the
/// transform engine cares about, such as legacy operators and convenience
/// macros). The result is a JSON object mapping each target key to its count,
/// suitable for corpus-level aggregation.
///
/// # Errors
///
/// Returns [`ParseError`](crate::ParseError) if the source does not parse into
/// a complete tree (no document, or a document containing parse-error nodes).
///
/// # Examples
///
/// ```
/// use texform::Parser;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let parser = Parser::builder().packages(&["base"]).build()?;
/// let counts = texform::analysis::count_targets(&parser, r"a \over b")?;
/// assert!(counts.is_object());
/// # Ok(())
/// # }
/// ```
pub fn count_targets(
    parser: &crate::Parser,
    src: &str,
) -> Result<serde_json::Value, crate::ParseError> {
    count_targets_from_output(parser.parse(src))
}

/// Count normalization targets in a formula with an explicit parse config.
///
/// Like [`count_targets`], but parses `src` with the supplied
/// [`ParseConfig`](crate::ParseConfig) instead of the parser default.
///
/// # Errors
///
/// Returns [`ParseError`](crate::ParseError) if the source does not parse into
/// a complete tree.
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
