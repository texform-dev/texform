//! `texform normalize`: normalize formulas with a transform profile.

use texform::bindings::NormalizeConfigInput;

use super::run_formulas;
use crate::input::{FormulaInput, read_config};
use crate::normalizer::{Normalizer, ProfileName, StyleName};
use crate::output::{Format, Success, ok_line, usage_error};

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    input: FormulaInput,

    /// Transform profile
    #[arg(long, value_enum)]
    profile: ProfileName,

    /// Output style
    #[arg(long, value_enum, default_value = "spaced")]
    style: StyleName,

    /// Overrides layered on the profile defaults, as JSON or `@FILE`
    #[arg(long, value_name = "JSON|@FILE")]
    config: Option<String>,

    /// Add the transform report to each result (requires `--json`)
    #[arg(long, requires = "json")]
    report: bool,

    /// Print one JSON object per formula
    #[arg(long)]
    json: bool,
}

pub fn run(args: Args, packages: &[String]) -> u8 {
    let overrides = match read_config::<NormalizeConfigInput>(args.config.as_deref()) {
        Ok(overrides) => overrides,
        Err(message) => return usage_error(message),
    };
    let normalizer = match Normalizer::build(args.profile, packages, overrides) {
        Ok(normalizer) => normalizer,
        Err(error) => return usage_error(error),
    };
    let format = if args.json {
        Format::Json
    } else {
        Format::Text
    };
    let style = texform::Style::from(args.style);
    run_formulas(&args.input, format, |latex| {
        let normalized = normalizer.normalize(latex, style, args.report)?;
        Ok(if args.json {
            Success::Json(ok_line(normalized))
        } else {
            Success::Text {
                line: normalized.output,
                warnings: Vec::new(),
            }
        })
    })
}
