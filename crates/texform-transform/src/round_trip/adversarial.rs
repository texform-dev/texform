//! Adversarial boundary inputs derived from the knowledge base and the
//! rewrite plans, so that new commands and rules are covered without a
//! handwritten list.
//!
//! Each input places a separator `S` that some profile deletes or replaces
//! between a capturer `C` and a successor `F`. When `S` goes away, `C` meets
//! `F` and could read it as an omitted trailing argument:
//!
//! - commands: `\C\S F`, where `C` takes only optional arguments;
//! - scoped commands: `\S\C\S F`, where `S` is a declarative that
//!   LowerAttributes consumes, so that the second `S` is redundant;
//! - environments: `\begin{E}\S F\end{E}`, where `E` takes only optional
//!   arguments;
//! - text mode: `\text{{\W} word}` and `\text{\W{} word}`, where `W` is a
//!   text-mode control word.
//!
//! Separators come from the triggers of every rule in the four profile plans
//! and from the declaratives and prefix wrappers that LowerAttributes
//! consumes; a prefix wrapper is written with an empty body, as in
//! `\mathbf{}`. Each input records
//! where its separator comes from, so that attribution tests can run one rule
//! or phase on the inputs it is responsible for.

use std::collections::{BTreeMap, BTreeSet};

use texform_core::parse::ParseContext;
use texform_knowledge::specs::{
    ArgForm, ArgSpec, CommandKind, ContentMode, DelimiterToken, ValueKind,
};

use crate::rewrite::RuleTarget;

/// Bumped whenever the generated input set changes for reasons other than
/// knowledge-base or rule changes.
pub const GENERATOR_VERSION: u32 = 2;

/// The separator source for declaratives that LowerAttributes consumes.
pub const LOWER_ATTRIBUTES: &str = "lower-attributes";

/// The separator source for prefix wrappers with an empty body, which
/// LowerAttributes removes. Kept apart from [`LOWER_ATTRIBUTES`] so that the
/// stratified subset covers both.
pub const LOWER_ATTRIBUTES_EMPTY_PREFIX: &str = "lower-attributes-empty-prefix";

/// Whether `source` names LowerAttributes rather than a rule.
pub fn is_lower_attributes(source: &str) -> bool {
    source == LOWER_ATTRIBUTES || source == LOWER_ATTRIBUTES_EMPTY_PREFIX
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Shape {
    Command,
    /// `\S\C\S F`, where the second declarative `S` is redundant.
    ScopedCommand,
    Environment,
    /// `\text{{\W} word}`.
    TextGroup,
    /// `\text{\W{} word}`.
    TextEmptyGroup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    /// The formula; also its stable identifier.
    pub source: String,
    pub shape: Shape,
    /// The capturer name: a command, an environment, or a text control word.
    pub capturer: String,
    /// The separator as written, empty for text shapes.
    pub separator: String,
    /// Rule keys, or a LowerAttributes source, that delete or replace the
    /// separator. Empty for text shapes.
    pub sources: Vec<String>,
    /// The successor as written, or `word` for text shapes.
    pub successor: String,
}

/// Every adversarial input for `parse_ctx`. Inputs that do not parse
/// completely are kept; callers skip them as they skip any incomplete input.
pub fn full(parse_ctx: &ParseContext) -> Vec<Input> {
    let separators = separators(parse_ctx);
    let math = parse_ctx.commands(ContentMode::Math);
    let capturers: Vec<_> = math
        .iter()
        .filter(|record| {
            record.kind != CommandKind::Infix && captures_anything(record.argspec.args)
        })
        .map(|record| (record.name, record.argspec.args))
        .collect();
    let environments: Vec<_> = parse_ctx
        .environments(ContentMode::Math)
        .into_iter()
        .filter(|record| captures_anything(record.argspec.args))
        .map(|record| (record.name, record.argspec.args))
        .collect();
    let mut successors = BTreeSet::new();
    for (_, args) in capturers.iter().chain(&environments) {
        for arg in *args {
            successors.extend(successors_for(&arg.form));
        }
    }
    successors.insert("{}^2".to_string());

    let declaratives: BTreeSet<String> = crate::lower_attributes::declarative_names()
        .map(|name| format!(r"\{name}"))
        .collect();
    let mut inputs = Vec::new();
    for (separator, sources) in &separators {
        for successor in &successors {
            for (name, _) in &capturers {
                inputs.push(Input {
                    source: format!(r"\{name}{separator}{successor}"),
                    shape: Shape::Command,
                    capturer: (*name).to_string(),
                    separator: separator.clone(),
                    sources: sources.clone(),
                    successor: successor.clone(),
                });
                if declaratives.contains(separator) {
                    inputs.push(Input {
                        source: format!(r"{separator}\{name}{separator}{successor}"),
                        shape: Shape::ScopedCommand,
                        capturer: (*name).to_string(),
                        separator: separator.clone(),
                        sources: vec![LOWER_ATTRIBUTES.to_string()],
                        successor: successor.clone(),
                    });
                }
            }
            for (name, _) in &environments {
                inputs.push(Input {
                    source: format!(r"\begin{{{name}}}{separator}{successor}\end{{{name}}}"),
                    shape: Shape::Environment,
                    capturer: (*name).to_string(),
                    separator: separator.clone(),
                    sources: sources.clone(),
                    successor: successor.clone(),
                });
            }
        }
    }
    if parse_ctx
        .lookup_command("text", ContentMode::Math)
        .is_some()
    {
        for record in parse_ctx.commands(ContentMode::Text) {
            let name = record.name;
            if !name.chars().all(|c| c.is_ascii_alphabetic())
                || record.kind == CommandKind::Infix
                || record.argspec.args.iter().any(|arg| arg.required)
            {
                continue;
            }
            for (shape, source) in [
                (Shape::TextGroup, format!(r"\text{{{{\{name}}} word}}")),
                (Shape::TextEmptyGroup, format!(r"\text{{\{name}{{}} word}}")),
            ] {
                inputs.push(Input {
                    source,
                    shape,
                    capturer: name.to_string(),
                    separator: String::new(),
                    sources: Vec::new(),
                    successor: "word".into(),
                });
            }
        }
    }
    inputs.sort_by(|a, b| a.source.cmp(&b.source));
    inputs.dedup_by(|a, b| a.source == b.source);
    inputs
}

/// A stratified subset of [`full`] for the fast tests: one input for each
/// combination of shape, separator source, and successor. Within a stratum,
/// the capturer and separator rotate so that strata cover different ones.
pub fn stratified(parse_ctx: &ParseContext) -> Vec<Input> {
    let mut strata: BTreeMap<(Shape, String, String), Vec<Input>> = BTreeMap::new();
    for input in full(parse_ctx) {
        let sources = if input.sources.is_empty() {
            vec![String::new()]
        } else {
            input.sources.clone()
        };
        for source in sources {
            strata
                .entry((input.shape, source, input.successor.clone()))
                .or_default()
                .push(input.clone());
        }
    }
    let mut picked: Vec<Input> = strata
        .into_values()
        .enumerate()
        .map(|(index, mut inputs)| {
            let pick = index.wrapping_mul(7919) % inputs.len();
            inputs.swap_remove(pick)
        })
        .collect();
    picked.sort_by(|a, b| a.source.cmp(&b.source));
    picked.dedup_by(|a, b| a.source == b.source);
    picked
}

/// Separators written with their required arguments, keyed by text, with the
/// rule keys or phase that delete or replace them.
fn separators(parse_ctx: &ParseContext) -> BTreeMap<String, Vec<String>> {
    let mut by_name: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
    for profile in super::PROFILES {
        let Ok(engine) = crate::TransformContext::from_build_config(
            crate::BuildConfig::profile(profile),
            parse_ctx,
        ) else {
            continue;
        };
        for rule in engine.rewrite_plan().rules() {
            let meta = rule.meta();
            for trigger in meta.triggers {
                let name = match trigger {
                    RuleTarget::Command(record) => record.name,
                    RuleTarget::Character(record) => record.name,
                    RuleTarget::Environment(_) => continue,
                };
                by_name
                    .entry(name)
                    .or_default()
                    .insert(meta.key.to_string());
            }
        }
    }
    let lower = crate::lower_attributes::declarative_names()
        .map(|name| (name, LOWER_ATTRIBUTES))
        .chain(
            crate::lower_attributes::prefix_names()
                .map(|name| (name, LOWER_ATTRIBUTES_EMPTY_PREFIX)),
        );
    for (name, source) in lower {
        by_name.entry(name).or_default().insert(source.to_string());
    }
    by_name
        .into_iter()
        .filter_map(|(name, sources)| {
            let record = parse_ctx.lookup_command(name, ContentMode::Math)?;
            let text = format!(r"\{name}{}", fillers(record.argspec.args));
            Some((text, sources.into_iter().collect()))
        })
        .collect()
}

/// Fills required arguments up to the first delimiter argument, which is left
/// for the successor to open, as in `\big[`.
fn fillers(args: &[ArgSpec]) -> String {
    let mut text = String::new();
    for arg in args.iter().filter(|arg| arg.required) {
        match arg.kind {
            ValueKind::Delimiter => break,
            ValueKind::Dimension => text.push_str("{1em}"),
            ValueKind::Integer => text.push_str("{2}"),
            _ => text.push_str("{}"),
        }
    }
    text
}

/// Whether a command or environment with no supplied arguments could read a
/// following sibling: all of its arguments are optional and one can open.
fn captures_anything(args: &[ArgSpec]) -> bool {
    !args.is_empty()
        && args.iter().all(|arg| !arg.required)
        && args
            .iter()
            .any(|arg| !matches!(arg.form, ArgForm::Until { .. }))
}

fn successors_for(form: &ArgForm) -> Vec<String> {
    let token = |token: &DelimiterToken| match token {
        DelimiterToken::Char(c) => c.to_string(),
        DelimiterToken::ControlSeq(name) if name.chars().all(|c| c.is_ascii_alphabetic()) => {
            format!(r"\{name} ")
        }
        DelimiterToken::ControlSeq(name) => format!(r"\{name}"),
    };
    match form {
        ArgForm::Standard => vec!["[x]".into()],
        ArgForm::Star => vec!["*".into()],
        ArgForm::Group => vec!["{x}".into()],
        ArgForm::Delimited { open, close } => vec![format!("{}x{}", token(open), token(close))],
        ArgForm::Paired { pairs } => pairs
            .iter()
            .map(|(open, close)| format!("{}x{}", token(open), token(close)))
            .collect(),
        ArgForm::Until { .. } => Vec::new(),
    }
}
