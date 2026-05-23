//! Target occurrence counting for parsed TeXForm syntax trees.
//!
//! Counts occurrences of command, environment, and character targets by
//! `(kind, content mode, name)`. Command-like nodes may contribute both command
//! and character counts when the knowledge base exposes the same name in both
//! categories.

use std::collections::HashMap;

use texform_interface::syntax_node::{Argument, ArgumentValue, ContentMode, SyntaxNode};
use texform_specs::builtin::ALL_PACKAGES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetKind {
    Cmd,
    Env,
    Char,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::Cmd => "cmd",
            TargetKind::Env => "env",
            TargetKind::Char => "char",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetCounterKey {
    pub kind: TargetKind,
    pub mode: ContentMode,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct TargetCounter {
    pub counts: HashMap<TargetCounterKey, u32>,
}

impl TargetCounter {
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    pub fn logical_counts(&self) -> HashMap<String, u32> {
        let mut out = HashMap::new();
        for (key, count) in &self.counts {
            let logical = format!("{}:{}", key.kind.as_str(), key.name);
            *out.entry(logical).or_insert(0) += *count;
        }
        out
    }

    pub fn bump(&mut self, kind: TargetKind, mode: ContentMode, name: &str) {
        let key = TargetCounterKey {
            kind,
            mode,
            name: name.to_string(),
        };
        *self.counts.entry(key).or_insert(0) += 1;
    }
}

/// Walk a `SyntaxNode` and accumulate target counts into `out`.
pub fn count_node(node: &SyntaxNode, out: &mut TargetCounter) {
    count_node_in_mode(node, ContentMode::Math, out);
}

fn count_node_in_mode(node: &SyntaxNode, inherited_mode: ContentMode, out: &mut TargetCounter) {
    match node {
        SyntaxNode::Root { mode, children } | SyntaxNode::Group { mode, children, .. } => {
            for child in children {
                count_node_in_mode(child, *mode, out);
            }
        }
        SyntaxNode::Command { name, args, .. } => {
            bump_cmd_like(out, inherited_mode, name);
            count_args(args, out);
        }
        SyntaxNode::Infix {
            name,
            args,
            left,
            right,
        } => {
            bump_cmd_like(out, inherited_mode, name);
            count_args(args, out);
            count_node_in_mode(left, inherited_mode, out);
            count_node_in_mode(right, inherited_mode, out);
        }
        SyntaxNode::Declarative { name, args } => {
            bump_cmd_like(out, inherited_mode, name);
            count_args(args, out);
        }
        SyntaxNode::Environment {
            name, args, body, ..
        } => {
            out.bump(TargetKind::Env, inherited_mode, name);
            count_args(args, out);
            count_node_in_mode(body, inherited_mode, out);
        }
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            count_node_in_mode(base, inherited_mode, out);
            if let Some(sub) = subscript {
                count_node_in_mode(sub, inherited_mode, out);
            }
            if let Some(sup) = superscript {
                count_node_in_mode(sup, inherited_mode, out);
            }
        }
        SyntaxNode::Text(_)
        | SyntaxNode::Char(_)
        | SyntaxNode::ActiveSpace
        | SyntaxNode::Error { .. } => {}
    }
}

fn bump_cmd_like(out: &mut TargetCounter, mode: ContentMode, name: &str) {
    let has_cmd = ALL_PACKAGES
        .iter()
        .any(|pkg| pkg.commands.iter().any(|record| record.name == name));
    let has_char = ALL_PACKAGES
        .iter()
        .any(|pkg| pkg.characters.iter().any(|record| record.name == name));

    if has_cmd || !has_char {
        out.bump(TargetKind::Cmd, mode, name);
    }
    if has_char {
        out.bump(TargetKind::Char, mode, name);
    }
}

fn count_args(args: &[Option<Argument>], out: &mut TargetCounter) {
    for slot in args {
        let Some(arg) = slot else { continue };
        match &arg.value {
            ArgumentValue::MathContent(node) => {
                count_node_in_mode(node, ContentMode::Math, out);
            }
            ArgumentValue::TextContent(node) => {
                count_node_in_mode(node, ContentMode::Text, out);
            }
            ArgumentValue::Delimiter(_)
            | ArgumentValue::CSName(_)
            | ArgumentValue::Dimension(_)
            | ArgumentValue::Integer(_)
            | ArgumentValue::KeyVal(_)
            | ArgumentValue::Column(_)
            | ArgumentValue::Boolean(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::parse::{ParseConfig, ParseContext};

    fn count(src: &str) -> HashMap<String, u32> {
        let output = ParseContext::shared().parse(src, &ParseConfig::default());
        let node = &output.result.expect("parse result").node;
        let mut counter = TargetCounter::default();
        count_node(node, &mut counter);
        counter.logical_counts()
    }

    #[test]
    fn counts_commands_envs_and_character_aliases() {
        let counts = count(r"\begin{matrix}\frac{a}{b} & x \le y\end{matrix}");

        assert_eq!(counts.get("env:matrix"), Some(&1));
        assert_eq!(counts.get("cmd:frac"), Some(&1));
        assert_eq!(counts.get("char:le"), Some(&1));
    }

    #[test]
    fn counts_text_mode_command_arguments() {
        let counts = count(r"\text{A \mkern 1em B}");

        assert_eq!(counts.get("cmd:text"), Some(&1));
        assert_eq!(counts.get("cmd:mkern"), Some(&1));
    }
}
