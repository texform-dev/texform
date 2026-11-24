//! `texform fragments`: split the serialization into typed fragments.

use texform::bindings::{
    NormalizeConfigInput, ParseConfigInput, SerializationFragmentDto, fragmented_latex_to_dto,
};
use texform::{Document, ParseConfig, Parser};

use super::run_formulas;
use crate::input::{FormulaInput, read_config};
use crate::normalizer::{Normalizer, ProfileName};
use crate::output::{Format, Success, json_line, usage_error};
use crate::packages;

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    input: FormulaInput,

    /// Normalize with this profile before splitting into fragments
    #[arg(long, value_enum)]
    profile: Option<ProfileName>,

    /// Parse options, or normalize overrides with `--profile`, as JSON or `@FILE`
    #[arg(long, value_name = "JSON|@FILE")]
    config: Option<String>,

    /// Print a JSON array of fragments, with text, kind, role, node path, slot, mode, and byte offsets
    #[arg(long)]
    json: bool,
}

/// How a formula becomes the document whose serialization is split into fragments.
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
                let normalizer = Normalizer::build(profile, packages, overrides)?;
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
    /// serialization. Fragment offsets refer to that serialization.
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

pub fn run(args: Args, packages: &[String]) -> u8 {
    let pipeline = match Pipeline::new(&args, packages) {
        Ok(pipeline) => pipeline,
        Err(message) => return usage_error(message),
    };
    let format = if args.json {
        Format::Json
    } else {
        Format::Text
    };
    run_formulas(&args.input, format, |latex| {
        let fragmented = pipeline
            .document(latex)?
            .to_latex_fragments()
            .map_err(texform::Error::from)?;
        let fragments = fragmented_latex_to_dto(fragmented).fragments;
        if args.json {
            return Ok(Success::Json(json_line(&fragments)));
        }
        Ok(Success::Text {
            line: describe_fragments(&fragments),
            warnings: Vec::new(),
        })
    })
}

fn describe_fragments(fragments: &[SerializationFragmentDto]) -> String {
    let items: Vec<String> = fragments
        .iter()
        .map(|fragment| {
            let kind = match fragment.kind {
                "character" => "char",
                "control_sequence" => "control_seq",
                "delimiter" => "delim",
                kind => kind,
            };
            let mode = if fragment.mode == "math" {
                String::new()
            } else {
                format!(", mode={}", fragment.mode)
            };
            format!("{kind}({}{mode})", json_line(&fragment.text))
        })
        .collect();
    format!("[{}]", items.join(", "))
}
