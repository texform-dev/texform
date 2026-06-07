//! Public TeXForm facade.
//!
//! This crate exposes the stable user-facing API: parse-only [`Parser`],
//! normalization [`TransformEngine`], AST serialization, argspec validation, and
//! analysis helpers.

pub mod analysis;
pub mod argspec;
#[doc(hidden)]
pub mod bindings;
pub mod config;
pub mod document;
pub mod error;
pub mod parse_result;
pub mod parser;
pub mod serialize;
pub mod transform_engine;

pub use argspec::{ValidateArgspecResult, validate_argspec};
pub use config::{NormalizeConfig, Profile, RuleKey, TransformConfig, rule_key_from_name};
pub use document::{
    ArgRef, ArgValue, DelimiterRef, DelimiterValue, Document, DocumentId, EditError,
    FromSyntaxError, GroupKindRef, NodeId, NodeKind, NodeRef,
};
pub use error::{Error, NormalizeError};
pub use parse_result::{ParseError, ParseResult};
pub use parser::{Parser, ParserBuildError, ParserBuilder};
pub use serialize::{SerializeError, SerializeOptions};
pub use transform_engine::{NormalizeResult, TransformEngine, TransformEngineBuilder};

pub use texform_core::parse::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveDelimiterRecord, ActiveEnvironmentRecord,
    AllowedMode, CommandItem, CommandKind, ContentMode, ContextItem, DelimiterControlItem,
    EnvironmentItem, PackageLoadError, ParseConfig, ParseDiagnostic, ParseDiagnosticContext,
    ParseDiagnosticKind, Span,
};
pub use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Delimiter, GroupKind, SyntaxNode,
};
pub use texform_transform::{
    Attr, AttrValue, AttributeFormCounts, AttributeSet, AttributeStat, FinalizeAstConfig,
    FinalizeAstReport, FinalizeAstStepReport, FinalizeAstStepReports, FlattenGroupsActionCounts,
    FlattenGroupsConfig, FlattenGroupsGuardCounts, FlattenGroupsReport, LowerAttributesConfig,
    LowerAttributesReport, MathFontValue, NormalizationLevelSet, RewriteReport, RewriteRuleStat,
    SizeValue, StyleValue, TextFamily, TextSeries, TextShape, TransformReport,
};
