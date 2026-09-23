//! `texform tokenize`: split the canonical serialization into typed tokens.

use std::process::ExitCode;

use texform::bindings::{NormalizeConfigInput, ParseConfigInput, tokenized_latex_to_dto};
use texform::{Document, ParseConfig, Parser};

use super::run_formulas;
use crate::input::{FormulaInput, read_config};
use crate::normalizer::{Normalizer, ProfileName};
use crate::output::{Success, ok_line, usage_error};
use crate::packages;

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    input: FormulaInput,

    /// Normalize with this profile before tokenizing
    #[arg(long, value_enum)]
    profile: Option<ProfileName>,

    /// Parse options, or normalize overrides with `--profile`, as JSON or `@FILE`
    #[arg(long, value_name = "JSON|@FILE")]
    config: Option<String>,

    /// Print one JSON object per formula, with token kinds and byte offsets
    #[arg(long)]
    json: bool,
}

/// How a formula becomes the document that is tokenized.
enum Pipeline {
    Parse { parser: Parser, config: ParseConfig },
    Normalize(Normalizer),
}

impl Pipeline {
    fn new(args: &Args, packages: &[String]) -> Result<Self, String> {
        let config = args.config.as_deref();
        match args.profile {
            Some(profile) => {
                let overrides = read_config::<NormalizeConfigInput>(config)?;
                let normalizer = Normalizer::build(profile, packages, overrides)
                    .map_err(|error| error.to_string())?;
                Ok(Self::Normalize(normalizer))
            }
            None => {
                let overrides = read_config::<ParseConfigInput>(config)?;
                let parser = packages::parser(packages).map_err(|error| error.to_string())?;
                let config = overrides.into_config(parser.default_parse_config().clone());
                Ok(Self::Parse { parser, config })
            }
        }
    }

    /// A complete document, normalized when a profile was given. The
    /// normalize path is `TransformEngine::normalize_with` stopped before
    /// serialization, so the tokens join to exactly its output.
    fn document(&self, latex: &str) -> Result<Document, texform::Error> {
        match self {
            Self::Parse { parser, config } => {
                Ok(parser.parse_with(latex, config).try_into_document()?.0)
            }
            Self::Normalize(Normalizer { engine, config }) => {
                let (mut document, _) = engine
                    .parser()
                    .parse_with(latex, &config.parse)
                    .try_into_document()?;
                engine.transform_with(&mut document, &config.transform)?;
                Ok(document)
            }
        }
    }
}

pub fn run(args: Args, packages: &[String]) -> ExitCode {
    let pipeline = match Pipeline::new(&args, packages) {
        Ok(pipeline) => pipeline,
        Err(message) => return usage_error(message),
    };
    run_formulas(&args.input, args.json, |latex| {
        let tokenized = pipeline
            .document(latex)?
            .to_tokenized_latex()
            .map_err(texform::Error::from)?;
        if args.json {
            return Ok(Success::Json(ok_line(tokenized_latex_to_dto(tokenized))));
        }
        let texts: Vec<&str> = tokenized
            .tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect();
        Ok(Success::Text {
            line: texts.join(" "),
            warnings: Vec::new(),
        })
    })
}
