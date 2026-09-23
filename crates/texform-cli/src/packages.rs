//! Knowledge package selection shared by subcommands.

/// Names of all built-in knowledge packages, in catalog order.
///
/// This is the default selection. It differs from the facade parser default,
/// which leaves `braket` out.
pub fn all() -> Vec<String> {
    texform::list_packages()
        .into_iter()
        .map(|package| package.name)
        .collect()
}

/// clap value parser for one `--packages` entry.
pub fn parse_name(name: &str) -> Result<String, String> {
    let known = all();
    if known.iter().any(|known| known == name) {
        Ok(name.to_owned())
    } else {
        Err(format!(
            "unknown package `{name}` (expected one of: {})",
            known.join(", ")
        ))
    }
}

/// Deduplicate `selection` into catalog order.
///
/// Parsers load packages in a fixed order regardless of how they were
/// requested, so this is the loaded set in a stable presentation.
pub fn canonical(selection: &[String]) -> Vec<String> {
    all()
        .into_iter()
        .filter(|name| selection.contains(name))
        .collect()
}

/// Build a parser that loads exactly `selection`.
pub fn parser(selection: &[String]) -> Result<texform::Parser, texform::ParserBuildError> {
    let names: Vec<&str> = selection.iter().map(String::as_str).collect();
    texform::Parser::builder().packages(&names).build()
}
