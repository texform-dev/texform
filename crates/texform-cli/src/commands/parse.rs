//! `texform parse`: parse formulas without normalizing them.

use std::process::ExitCode;

use serde::Serialize;
use texform::bindings::{ParseConfigInput, normalize_error_to_parts};
use texform::{ParseDiagnostic, SyntaxNode};

use super::run_formulas;
use crate::input::{FormulaInput, read_config};
use crate::output::{Failure, Success, ok_line, usage_error};
use crate::packages;

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    input: FormulaInput,

    /// Parse options (`reject_unknown`, `abort_on_error`, `max_group_depth`), as JSON or `@FILE`
    #[arg(long, value_name = "JSON|@FILE")]
    config: Option<String>,

    /// Print one JSON object per formula, with the syntax tree
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
struct Parsed {
    syntax: SyntaxNode,
    diagnostics: Vec<ParseDiagnostic>,
}

pub fn run(args: Args, packages: &[String]) -> ExitCode {
    let overrides = match read_config::<ParseConfigInput>(args.config.as_deref()) {
        Ok(overrides) => overrides,
        Err(message) => return usage_error(message),
    };
    let parser = match packages::parser(packages) {
        Ok(parser) => parser,
        Err(error) => return usage_error(error),
    };
    let config = overrides.into_config(parser.default_parse_config().clone());
    run_formulas(&args.input, args.json, |latex| {
        // Success means a complete document, the same condition normalize
        // requires; a partial document with error nodes is a failure.
        match parser.parse_with(latex, &config).try_into_document() {
            Ok((document, diagnostics)) if args.json => Ok(Success::Json(ok_line(Parsed {
                syntax: document.to_syntax(),
                diagnostics,
            }))),
            Ok((document, diagnostics)) => Ok(Success::Text {
                line: document.to_latex().map_err(texform::Error::from)?,
                warnings: diagnostics,
            }),
            Err(error) => {
                let parts = normalize_error_to_parts(error.into());
                let syntax = parts
                    .document
                    .filter(|_| args.json)
                    .map(|document| Box::new(document.to_syntax()));
                Err(Failure {
                    error: parts.error,
                    syntax,
                })
            }
        }
    })
}
