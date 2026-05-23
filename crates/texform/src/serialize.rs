use texform_core::ast::Ast;
pub use texform_core::serialize::SerializeOptions;
use texform_interface::syntax_node::SyntaxNode;

#[derive(Debug)]
pub enum SerializeError {
    ExpectedRoot,
}

impl std::fmt::Display for SerializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExpectedRoot => f.write_str("serialize expects a syntax root"),
        }
    }
}

impl std::error::Error for SerializeError {}

pub trait SerializeInput {
    fn to_ast_for_serialize(&self) -> Result<Ast, SerializeError>;
}

impl SerializeInput for Ast {
    fn to_ast_for_serialize(&self) -> Result<Ast, SerializeError> {
        Ok(self.clone())
    }
}

impl SerializeInput for SyntaxNode {
    fn to_ast_for_serialize(&self) -> Result<Ast, SerializeError> {
        match self {
            SyntaxNode::Root { .. } => Ok(Ast::from_syntax_root(self)),
            _ => Err(SerializeError::ExpectedRoot),
        }
    }
}

pub fn serialize(input: &impl SerializeInput) -> Result<String, SerializeError> {
    serialize_with(input, &SerializeOptions::default())
}

pub fn serialize_with(
    input: &impl SerializeInput,
    options: &SerializeOptions,
) -> Result<String, SerializeError> {
    let ast = input.to_ast_for_serialize()?;
    Ok(texform_core::serialize::serialize_with(&ast, options))
}
