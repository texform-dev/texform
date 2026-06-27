#![deny(missing_docs)]
//! Public TeXForm facade — parse, edit, serialize, and normalize LaTeX math.
//!
//! `texform` is the only crate in the workspace with a public stability
//! guarantee. Every other `texform-*` crate is an internal implementation
//! detail whose API may change without notice; external code should depend on
//! this facade alone. See the project `ARCHITECTURE.md` for the full crate
//! layout and the pipeline a formula travels through.
//!
//! # The two entry points
//!
//! - [`Parser`] parses LaTeX into an editable [`Document`] without normalizing
//!   it. Use it when you want to inspect or mutate the tree yourself.
//! - [`TransformEngine`] pairs a parser with a normalization pipeline selected
//!   by a [`Profile`]. Use it to canonicalize formulas for a downstream
//!   scenario (authoring output, corpus normalization, equivalence comparison).
//!
//! A formula flows in one direction: source text parses into a [`Document`]
//! (a DOM-style tree wrapping an internal arena), which you can query, edit,
//! serialize back to LaTeX with [`Document::to_latex`], convert to a
//! [`SyntaxNode`] for serde with [`Document::to_syntax`], or normalize in place
//! with [`TransformEngine::transform`].
//!
//! # Error model and tree states
//!
//! Parsing never panics on bad input. It produces a [`ParseResult`] with one of
//! three honest states: no tree, a complete editable tree, or a partial
//! read-only tree carrying [`Error`](SyntaxNode) placeholder nodes. Whether a
//! tree contains such placeholders is the O(1) [`Document::has_errors`] signal,
//! which is distinct from the [`ParseConfig::abort_on_error`] parse-strictness
//! knob. A tree with errors is read-only and cannot be normalized.
//!
//! # Example
//!
//! Normalize a legacy `\over` fraction with the corpus profile, then parse,
//! edit-free transform, and serialize the same input through the live document:
//!
//! ```
//! use texform::{Profile, TransformEngine};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = TransformEngine::builder().profile(Profile::Corpus).build()?;
//! let result = engine.normalize(r"a \over b")?;
//! assert_eq!(result.normalized, r"\frac { a } { b }");
//!
//! let (mut document, _) = engine.parser().parse(r"a \over b").try_into_document()?;
//! engine.transform(&mut document)?;
//! assert_eq!(document.to_latex()?, r"\frac { a } { b }");
//! # Ok(())
//! # }
//! ```

pub mod analysis;
pub mod argspec;
#[doc(hidden)]
pub mod bindings;
pub mod config;
pub mod document;
pub mod error;
pub mod knowledge;
pub mod parse_result;
pub mod parser;
pub mod serialize;
pub mod transform_engine;

pub use argspec::{
    ArgSpecFormInfo, ArgSpecKindInfo, DelimiterTokenInfo, DelimiterTokenPairInfo,
    ParsedArgSpecSlot, RuntimeContentModeInfo, ValidateArgspecResult, validate_argspec,
};
pub use config::{NormalizeConfig, Profile, RuleKey, TransformConfig, rule_key_from_name};
pub use document::{
    ArgRef, ArgValue, DelimiterRef, DelimiterValue, Document, DocumentId, EditError,
    FromSyntaxError, GroupKindRef, NodeId, NodeKind, NodeRef, NodeSpanEntry,
};
pub use error::{Error, NormalizeError, TransformBuildError, TransformError};
pub use knowledge::{PackageInfo, list_packages};
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
