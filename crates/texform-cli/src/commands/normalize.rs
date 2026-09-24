//! `texform normalize`: normalize formulas with a transform profile.

use std::process::ExitCode;

use serde::Serialize;
use texform::bindings::{NormalizeConfigInput, TransformReportDto, transform_report_to_dto};

use super::run_formulas;
use crate::input::{FormulaInput, read_config};
use crate::normalizer::{Normalizer, ProfileName};
use crate::output::{Format, Success, ok_line, usage_error};

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    input: FormulaInput,

    /// Transform profile
    #[arg(long, value_enum)]
    profile: ProfileName,

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

#[derive(Serialize)]
struct Normalized {
    output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<TransformReportDto>,
}

pub fn run(args: Args, packages: &[String]) -> ExitCode {
    let overrides = match read_config::<NormalizeConfigInput>(args.config.as_deref()) {
        Ok(overrides) => overrides,
        Err(message) => return usage_error(message),
    };
    let Normalizer { engine, config } = match Normalizer::build(args.profile, packages, overrides) {
        Ok(normalizer) => normalizer,
        Err(error) => return usage_error(error),
    };
    let format = if args.json {
        Format::Json
    } else {
        Format::Text
    };
    run_formulas(&args.input, format, |latex| {
        if args.report {
            let result = engine.normalize_with_report(latex, &config)?;
            return Ok(Success::Json(ok_line(Normalized {
                output: result.normalized,
                report: Some(transform_report_to_dto(&result.report)),
            })));
        }
        let output = engine.normalize_with(latex, &config)?;
        Ok(if args.json {
            Success::Json(ok_line(Normalized {
                output,
                report: None,
            }))
        } else {
            Success::Text {
                line: output,
                warnings: Vec::new(),
            }
        })
    })
}
