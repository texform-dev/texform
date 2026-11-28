//! Knowledge package selection shared by subcommands.

/// Names of all built-in knowledge packages, in catalog order.
pub fn all() -> Vec<String> {
    texform::list_packages()
        .into_iter()
        .map(|package| package.name)
        .collect()
}

/// The facade's runtime defaults: physics is opt-in and braket is enabled.
pub fn defaults() -> Vec<String> {
    texform::default_packages()
        .into_iter()
        .map(str::to_owned)
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

/// Build a parser that loads exactly `selection`.
pub fn parser(selection: &[String]) -> Result<texform::Parser, texform::KnowledgeBaseBuildError> {
    let names: Vec<&str> = selection.iter().map(String::as_str).collect();
    let knowledge_base = texform::KnowledgeBase::builder().packages(&names).build()?;
    Ok(texform::Parser::builder()
        .knowledge_base(knowledge_base)
        .build())
}
