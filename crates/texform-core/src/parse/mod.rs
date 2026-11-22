mod config;
mod context;
pub(crate) mod cursor;
pub(crate) mod diagnostics;
mod grammar;
pub(crate) mod parser;
pub(crate) mod recovery;
mod result;
pub(crate) mod tree;

use crate::document::Document;
use crate::lexer::lex_source;
use crate::parse::cursor::Cursor;
use crate::parse::diagnostics::finish_diagnostics;
use crate::parse::parser::TokenParser;

pub use crate::knowledge::default_package_names;
pub use config::ParseConfig;
pub use context::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveDelimiterRecord, ActiveEnvironmentRecord,
    AllowedMode, ArgSpecParseError, CommandItem, CommandKind, ContentMode, ContextItem,
    DelimiterControlItem, EnvironmentItem, KnowledgeBase, KnowledgeBaseBuildError,
    KnowledgeBaseBuilder, MutationSummary, PackageLoadError, ParseContext, ParseContextBuildError,
    ParseContextBuilder,
};
pub(crate) use grammar::scalar::validate_scalar_argument;
pub use result::{
    ParseDiagnostic, ParseDiagnosticContext, ParseDiagnosticKind, ParseError, ParseResult, Span,
};

/// Parse `src` in `mode` using the recursive-descent parser.
pub(crate) fn parse_with_context_mode(
    ctx: &ParseContext,
    src: &str,
    config: &ParseConfig,
    mode: ContentMode,
) -> ParseResult {
    let stream = match lex_source(src) {
        Ok(stream) => stream,
        Err(diag) => {
            return ParseResult {
                document: None,
                diagnostics: vec![diag],
            };
        }
    };

    let cursor = Cursor::from_stream(&stream, src.len());
    let mut parser = TokenParser::new(cursor, src, ctx, config);
    let output = parser.parse_root(mode).ok();
    let diagnostics = finish_diagnostics(parser.into_diagnostics());

    let document = output.map(|node| {
        let (syntax, span_tree) = node.finish_root();
        Document::from_syntax_with_spans(ctx, &syntax, &span_tree)
    });

    ParseResult {
        document,
        diagnostics,
    }
}
