//! AST serializer scaffold.

use crate::ast::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind,
    Node, NodeId,
};

pub fn serialize(ast: &Ast) -> String {
    serialize_with(ast, &SerializeOptions::default())
}

pub fn serialize_with(ast: &Ast, options: &SerializeOptions) -> String {
    let mut serializer = Serializer::new(ast, options);
    serializer.serialize_root();
    serializer.finish()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SerializeOptions {
    pub math: MathSerializeOptions,
    pub syntax: SyntaxSerializeOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MathSerializeOptions {
    pub spacing: MathSpacingOptions,
    pub scripts: MathScriptOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MathSpacingOptions {
    pub commands: CommandSpacing,
    pub group_inner_spacing: MathGroupInnerSpacing,
    pub adjacent_chars: AdjacentCharSpacing,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MathScriptOptions {
    pub grouping: ScriptGrouping,
    pub spacing: ScriptSpacing,
    pub order: ScriptOrder,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyntaxSerializeOptions {
    pub arguments: ArgumentSerializeOptions,
    pub environments: EnvironmentSerializeOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArgumentSerializeOptions {
    pub grouping: ArgumentGrouping,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvironmentSerializeOptions {
    pub name_spacing: EnvironmentNameSpacing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandSpacing {
    #[default]
    Spaced,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MathGroupInnerSpacing {
    #[default]
    Padded,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdjacentCharSpacing {
    #[default]
    Spaced,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScriptGrouping {
    #[default]
    AlwaysExplicit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScriptSpacing {
    #[default]
    Spaced,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScriptOrder {
    #[default]
    SubFirst,
    SupFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArgumentGrouping {
    #[default]
    AlwaysExplicit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnvironmentNameSpacing {
    #[default]
    Spaced,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomKind {
    ControlSequence,
    TextChunk,
    MathChar,
    Brace,
    DelimiterToken,
    ScriptMark,
    Dollar,
    ActiveChar,
    RawFragment,
}

#[derive(Default)]
struct AtomWriter {
    output: String,
    previous: Option<AtomKind>,
}

impl AtomWriter {
    fn emit(&mut self, mode: ContentMode, kind: AtomKind, text: &str, options: &SerializeOptions) {
        if self.should_insert_space(mode, kind, options) {
            self.output.push(' ');
        }
        self.output.push_str(text);
        self.previous = Some(kind);
    }

    fn emit_star_suffix(&mut self) {
        self.output.push('*');
    }

    fn should_insert_space(
        &self,
        mode: ContentMode,
        next: AtomKind,
        options: &SerializeOptions,
    ) -> bool {
        let Some(prev) = self.previous else {
            return false;
        };

        if matches!(prev, AtomKind::ControlSequence)
            && matches!(
                next,
                AtomKind::TextChunk | AtomKind::MathChar | AtomKind::RawFragment
            )
        {
            return true;
        }

        if matches!(mode, ContentMode::Text) {
            return false;
        }

        if matches!(prev, AtomKind::ControlSequence) {
            return match next {
                AtomKind::Brace => matches!(options.math.spacing.commands, CommandSpacing::Spaced),
                _ => true,
            };
        }

        if matches!(prev, AtomKind::MathChar) && matches!(next, AtomKind::MathChar) {
            return matches!(
                options.math.spacing.adjacent_chars,
                AdjacentCharSpacing::Spaced
            );
        }

        if matches!(prev, AtomKind::Dollar) || matches!(next, AtomKind::Dollar) {
            return false;
        }

        if matches!(prev, AtomKind::ScriptMark) || matches!(next, AtomKind::ScriptMark) {
            return matches!(options.math.scripts.spacing, ScriptSpacing::Spaced);
        }

        true
    }

    fn finish(self) -> String {
        self.output
    }
}

struct Serializer<'a> {
    ast: &'a Ast,
    options: &'a SerializeOptions,
    writer: AtomWriter,
}

impl<'a> Serializer<'a> {
    fn new(ast: &'a Ast, options: &'a SerializeOptions) -> Self {
        Self {
            ast,
            options,
            writer: AtomWriter::default(),
        }
    }

    fn serialize_root(&mut self) {
        let root = self.ast.root();
        let Node::Group { children, mode, .. } = self.ast.node(root) else {
            unreachable!("root must be a group")
        };

        for &child in children {
            self.visit(child, *mode);
        }
    }

    fn visit(&mut self, id: NodeId, mode: ContentMode) {
        match self.ast.node(id).clone() {
            Node::Environment { name, args, body } => {
                self.visit_environment(&name, &args, body, mode)
            }
            Node::Infix {
                name,
                args,
                left,
                right,
            } => self.visit_infix(&name, &args, left, right),
            Node::Declarative { name, args, scope } => {
                self.visit_declarative(&name, &args, scope, mode)
            }
            Node::Group {
                children,
                kind,
                mode: child_mode,
            } => self.visit_group(id, kind, child_mode, &children),
            Node::Scripted {
                base,
                subscript,
                superscript,
            } => self.visit_scripted(base, subscript, superscript),
            Node::Command { name, args } => self.visit_command(&name, &args, mode),
            Node::Char(ch) => self.visit_char(ch, mode),
            Node::Text(text) => self
                .writer
                .emit(mode, AtomKind::TextChunk, &text, self.options),
            Node::ActiveSpace => self
                .writer
                .emit(mode, AtomKind::ActiveChar, "~", self.options),
            Node::UnknownCommand { name } => self.writer.emit(
                mode,
                AtomKind::ControlSequence,
                &format!(r"\{}", name),
                self.options,
            ),
        }
    }

    fn visit_group(
        &mut self,
        id: NodeId,
        kind: GroupKind,
        child_mode: ContentMode,
        children: &[NodeId],
    ) {
        match kind {
            GroupKind::Explicit | GroupKind::Implicit if id == self.ast.root() => {
                for &child in children {
                    self.visit(child, child_mode);
                }
            }
            GroupKind::Explicit | GroupKind::Implicit => {
                self.emit_wrapped(child_mode, AtomKind::Brace, "{", "}", children);
            }
            GroupKind::Delimited { left, right } => {
                self.writer.emit(
                    ContentMode::Math,
                    AtomKind::ControlSequence,
                    r"\left",
                    self.options,
                );
                self.emit_delimiter(&left, ContentMode::Math);
                for &child in children {
                    self.visit(child, ContentMode::Math);
                }
                self.writer.emit(
                    ContentMode::Math,
                    AtomKind::ControlSequence,
                    r"\right",
                    self.options,
                );
                self.emit_delimiter(&right, ContentMode::Math);
            }
            GroupKind::InlineMath => self.visit_inline_math(children),
        }
    }

    fn visit_command(&mut self, name: &str, args: &[Option<Argument>], mode: ContentMode) {
        self.writer.emit(
            mode,
            AtomKind::ControlSequence,
            &format!(r"\{}", name),
            self.options,
        );

        for slot in args {
            self.visit_argument_slot(slot, mode);
        }
    }

    fn visit_infix(&mut self, name: &str, args: &[ArgumentSlot], left: NodeId, right: NodeId) {
        self.visit(left, ContentMode::Math);
        self.writer.emit(
            ContentMode::Math,
            AtomKind::ControlSequence,
            &format!(r"\{}", name),
            self.options,
        );
        for slot in args {
            self.visit_argument_slot(slot, ContentMode::Math);
        }
        self.visit(right, ContentMode::Math);
    }

    fn visit_declarative(
        &mut self,
        name: &str,
        args: &[ArgumentSlot],
        scope: NodeId,
        mode: ContentMode,
    ) {
        self.writer.emit(
            mode,
            AtomKind::ControlSequence,
            &format!(r"\{}", name),
            self.options,
        );
        for slot in args {
            self.visit_argument_slot(slot, mode);
        }
        self.visit(scope, mode);
    }

    fn visit_environment(
        &mut self,
        name: &str,
        args: &[ArgumentSlot],
        body: NodeId,
        mode: ContentMode,
    ) {
        self.emit_environment_head(mode, r"\begin", name);
        for slot in args {
            self.visit_argument_slot(slot, mode);
        }

        match self.ast.node(body).clone() {
            Node::Group {
                children,
                mode: body_mode,
                kind: GroupKind::Implicit,
            } => {
                for child in children {
                    self.visit(child, body_mode);
                }
            }
            Node::Group {
                mode: body_mode, ..
            } => self.visit(body, body_mode),
            other => unreachable!("environment body must remain a group, got {:?}", other),
        }

        self.emit_environment_head(mode, r"\end", name);
    }

    fn emit_environment_head(&mut self, outer_mode: ContentMode, head: &str, name: &str) {
        self.writer
            .emit(outer_mode, AtomKind::ControlSequence, head, self.options);

        let brace_mode = match self.options.syntax.environments.name_spacing {
            EnvironmentNameSpacing::Spaced => ContentMode::Math,
            EnvironmentNameSpacing::Compact => ContentMode::Text,
        };
        self.writer
            .emit(brace_mode, AtomKind::Brace, "{", self.options);
        self.writer
            .emit(ContentMode::Text, AtomKind::TextChunk, name, self.options);
        self.writer
            .emit(ContentMode::Text, AtomKind::Brace, "}", self.options);
    }

    fn visit_argument_slot(&mut self, slot: &Option<Argument>, mode: ContentMode) {
        let Some(arg) = slot else {
            return;
        };

        match (&arg.kind, &arg.value) {
            (ArgumentKind::Star, ArgumentValue::Boolean(true)) => self.writer.emit_star_suffix(),
            (ArgumentKind::Star, ArgumentValue::Boolean(false)) => {}
            (ArgumentKind::Star, _) => {
                unreachable!("star slots must carry boolean values")
            }
            (ArgumentKind::Mandatory | ArgumentKind::Group, ArgumentValue::MathContent(child)) => {
                self.emit_argument_content(*child, ContentMode::Math, "{", "}", mode);
            }
            (ArgumentKind::Mandatory | ArgumentKind::Group, ArgumentValue::TextContent(child)) => {
                self.emit_argument_content(*child, ContentMode::Text, "{", "}", mode);
            }
            (ArgumentKind::Optional, ArgumentValue::MathContent(child)) => {
                self.emit_argument_content(*child, ContentMode::Math, "[", "]", mode);
            }
            (ArgumentKind::Optional, ArgumentValue::TextContent(child)) => {
                self.emit_argument_content(*child, ContentMode::Text, "[", "]", mode);
            }
            (ArgumentKind::Mandatory | ArgumentKind::Group, value) => {
                self.emit_scalar_wrapped(value, "{", "}", mode)
            }
            (ArgumentKind::Optional, value) => self.emit_scalar_wrapped(value, "[", "]", mode),
            (ArgumentKind::Delimited { open, close }, ArgumentValue::MathContent(node))
            | (ArgumentKind::Paired { open, close }, ArgumentValue::MathContent(node)) => {
                self.emit_recorded_delimiters(open, close, *node, ContentMode::Math)
            }
            (ArgumentKind::Delimited { open, close }, ArgumentValue::TextContent(node))
            | (ArgumentKind::Paired { open, close }, ArgumentValue::TextContent(node)) => {
                self.emit_recorded_delimiters(open, close, *node, ContentMode::Text)
            }
            (ArgumentKind::Delimited { open, close }, value)
            | (ArgumentKind::Paired { open, close }, value) => {
                self.emit_scalar_between_delimiters(open, close, value, mode)
            }
        }
    }

    fn emit_argument_content(
        &mut self,
        child: NodeId,
        content_mode: ContentMode,
        open: &str,
        close: &str,
        wrapper_mode: ContentMode,
    ) {
        self.emit_wrapped_content(child, wrapper_mode, content_mode, open, close);
    }

    fn visit_scripted(
        &mut self,
        base: NodeId,
        subscript: Option<NodeId>,
        superscript: Option<NodeId>,
    ) {
        self.visit(base, ContentMode::Math);

        match self.options.math.scripts.order {
            ScriptOrder::SubFirst => {
                if let Some(node) = subscript {
                    self.emit_script('_', node);
                }
                if let Some(node) = superscript {
                    self.emit_script('^', node);
                }
            }
            ScriptOrder::SupFirst => {
                if let Some(node) = superscript {
                    self.emit_script('^', node);
                }
                if let Some(node) = subscript {
                    self.emit_script('_', node);
                }
            }
        }
    }

    fn visit_inline_math(&mut self, children: &[NodeId]) {
        self.writer
            .emit(ContentMode::Text, AtomKind::Dollar, "$", self.options);
        for &child in children {
            self.visit(child, ContentMode::Math);
        }
        self.writer
            .emit(ContentMode::Text, AtomKind::Dollar, "$", self.options);
    }

    fn emit_script(&mut self, marker: char, node: NodeId) {
        let mode = match self.options.math.scripts.spacing {
            ScriptSpacing::Spaced => ContentMode::Math,
            ScriptSpacing::Compact => ContentMode::Text,
        };
        self.writer.emit(
            mode,
            AtomKind::ScriptMark,
            &marker.to_string(),
            self.options,
        );
        self.emit_wrapped_content(node, ContentMode::Math, ContentMode::Math, "{", "}");
    }

    fn emit_wrapped(
        &mut self,
        mode: ContentMode,
        kind: AtomKind,
        open: &str,
        close: &str,
        children: &[NodeId],
    ) {
        if children.is_empty()
            && matches!(mode, ContentMode::Math)
            && matches!(kind, AtomKind::Brace)
            && matches!(
                self.options.math.spacing.group_inner_spacing,
                MathGroupInnerSpacing::Padded
            )
        {
            self.emit_padded_empty_group(mode, kind, open, close);
            return;
        }

        self.writer.emit(mode, kind, open, self.options);
        for &child in children {
            self.visit(child, mode);
        }
        self.writer.emit(mode, kind, close, self.options);
    }

    fn emit_padded_empty_group(
        &mut self,
        mode: ContentMode,
        kind: AtomKind,
        open: &str,
        close: &str,
    ) {
        if self.writer.should_insert_space(mode, kind, self.options) {
            self.writer.output.push(' ');
        }
        self.writer.output.push_str(open);
        self.writer.output.push(' ');
        self.writer.output.push_str(close);
        self.writer.previous = Some(kind);
    }

    fn emit_wrapped_content(
        &mut self,
        child: NodeId,
        wrapper_mode: ContentMode,
        content_mode: ContentMode,
        open: &str,
        close: &str,
    ) {
        self.writer
            .emit(wrapper_mode, AtomKind::Brace, open, self.options);

        match self.ast.node(child) {
            // Wrapper-owned braces should not duplicate content groups.
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                mode: child_mode,
            } => {
                if children.is_empty()
                    && matches!(*child_mode, ContentMode::Math)
                    && matches!(
                        self.options.math.spacing.group_inner_spacing,
                        MathGroupInnerSpacing::Padded
                    )
                {
                    self.writer.output.push(' ');
                    self.writer.output.push_str(close);
                    self.writer.previous = Some(AtomKind::Brace);
                    return;
                }
                for &grandchild in children {
                    self.visit(grandchild, *child_mode);
                }
            }
            _ => self.visit(child, content_mode),
        }

        self.writer
            .emit(content_mode, AtomKind::Brace, close, self.options);
    }

    fn emit_scalar_wrapped(
        &mut self,
        value: &ArgumentValue,
        open: &str,
        close: &str,
        mode: ContentMode,
    ) {
        if self
            .writer
            .should_insert_space(mode, AtomKind::Brace, self.options)
        {
            self.writer.output.push(' ');
        }
        self.writer.output.push_str(open);
        self.writer
            .output
            .push_str(&self.scalar_argument_text(value));
        self.writer.output.push_str(close);
        self.writer.previous = Some(AtomKind::Brace);
    }

    fn emit_recorded_delimiters(
        &mut self,
        open: &Delimiter,
        close: &Delimiter,
        node: NodeId,
        mode: ContentMode,
    ) {
        self.emit_delimiter(open, mode);
        self.visit_argument_content_node(node, mode);
        self.emit_delimiter(close, mode);
    }

    fn emit_scalar_between_delimiters(
        &mut self,
        open: &Delimiter,
        close: &Delimiter,
        value: &ArgumentValue,
        mode: ContentMode,
    ) {
        self.emit_delimiter(open, mode);
        let text = self.scalar_argument_text(value);
        self.writer
            .emit(mode, AtomKind::RawFragment, &text, self.options);
        self.emit_delimiter(close, mode);
    }

    fn scalar_argument_text(&self, value: &ArgumentValue) -> String {
        match value {
            ArgumentValue::Delimiter(delimiter) => self.delimiter_text(delimiter),
            ArgumentValue::CSName(name)
            | ArgumentValue::Dimension(name)
            | ArgumentValue::Integer(name)
            | ArgumentValue::KeyVal(name)
            | ArgumentValue::Column(name) => name.clone(),
            ArgumentValue::Boolean(_) => {
                unreachable!("boolean values are only valid in star slots")
            }
            ArgumentValue::MathContent(_) | ArgumentValue::TextContent(_) => {
                unreachable!("content variants must be serialized as child nodes")
            }
        }
    }

    fn delimiter_text(&self, delimiter: &Delimiter) -> String {
        match delimiter {
            Delimiter::None => ".".to_string(),
            Delimiter::Char(ch) => ch.to_string(),
            Delimiter::Control(name) => format!(r"\{}", name),
        }
    }

    fn emit_delimiter(&mut self, delimiter: &Delimiter, mode: ContentMode) {
        match delimiter {
            Delimiter::None => self
                .writer
                .emit(mode, AtomKind::DelimiterToken, ".", self.options),
            Delimiter::Char(ch) => self.writer.emit(
                mode,
                AtomKind::DelimiterToken,
                &ch.to_string(),
                self.options,
            ),
            Delimiter::Control(name) => self.writer.emit(
                mode,
                AtomKind::DelimiterToken,
                &format!(r"\{}", name),
                self.options,
            ),
        }
    }

    fn visit_argument_content_node(&mut self, node: NodeId, mode: ContentMode) {
        match self.ast.node(node) {
            Node::Group {
                children,
                kind: GroupKind::Explicit | GroupKind::Implicit,
                mode: child_mode,
            } => {
                for &child in children {
                    self.visit(child, *child_mode);
                }
            }
            _ => self.visit(node, mode),
        }
    }

    fn visit_char(&mut self, ch: char, mode: ContentMode) {
        let kind = if matches!(mode, ContentMode::Text) {
            AtomKind::TextChunk
        } else {
            AtomKind::MathChar
        };
        self.writer.emit(mode, kind, &ch.to_string(), self.options);
    }

    fn finish(self) -> String {
        self.writer.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atom_writer_glues_star_to_control_sequence() {
        let options = SerializeOptions::default();
        let mut writer = AtomWriter::default();

        writer.emit(
            ContentMode::Math,
            AtomKind::ControlSequence,
            r"\operatorname",
            &options,
        );
        writer.emit_star_suffix();

        assert_eq!(writer.finish(), r"\operatorname*");
    }

    #[test]
    fn test_atom_writer_keeps_text_chunk_compact() {
        let options = SerializeOptions::default();
        let mut writer = AtomWriter::default();

        writer.emit(ContentMode::Text, AtomKind::TextChunk, "abc", &options);
        writer.emit(ContentMode::Text, AtomKind::TextChunk, " def", &options);

        assert_eq!(writer.finish(), "abc def");
    }
}
