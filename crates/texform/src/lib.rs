//! Public TeXForm facade.
//!
//! This crate exposes the stable user-facing API: parse-only [`Parser`],
//! normalization [`Engine`], AST serialization, argspec validation, and
//! analysis helpers.

pub mod analysis;
pub mod argspec;
pub mod config;
pub mod engine;
pub mod error;
pub mod parser;
pub mod serialize;

pub use analysis::{count_targets, count_targets_with};
pub use argspec::{ValidateArgspecResult, validate_argspec};
pub use config::{NormalizeConfig, Profile, RuleKey, TransformConfig, rule_key_from_name};
pub use engine::{Engine, EngineBuilder, NormalizeResult};
pub use error::{Error, NormalizeError};
pub use parser::{Parser, ParserBuildError, ParserBuilder};
pub use serialize::{SerializeError, SerializeInput, SerializeOptions, serialize, serialize_with};

pub use texform_core::ast::Ast;
pub use texform_core::parse::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveDelimiterRecord, ActiveEnvironmentRecord,
    AllowedMode, CommandItem, CommandKind, ContentMode, ContextItem, DelimiterControlItem,
    EnvironmentItem, PackageLoadError, ParseAstError, ParseConfig, ParseDiagnostic,
    ParseDiagnosticContext, ParseDiagnosticKind, ParseOutput, ParseResult, Span,
};
pub use texform_interface::syntax_node::SyntaxNode;
pub use texform_transform::{
    FlattenGroupsConfig, FlattenGroupsReport, LowerAttributesConfig, LowerAttributesReport,
    RuleClassSet, TransformReport,
};
