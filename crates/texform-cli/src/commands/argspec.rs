//! `texform argspec`: argument-specification tools.

use std::process::ExitCode;

use texform::{
    ArgSpecFormInfo, ArgSpecKindInfo, DelimiterTokenInfo, ParsedArgSpecSlot,
    RuntimeContentModeInfo, validate_argspec,
};

use crate::output::{FAILURE, eprint_line, io_error, json_line, print_line};

#[derive(clap::Subcommand)]
pub enum Command {
    /// Check an xparse-style argument specification and describe its slots
    Validate(ValidateArgs),
}

#[derive(clap::Args)]
pub struct ValidateArgs {
    /// Argument specification, for example `m O{default} m`
    #[arg(value_name = "SPEC", allow_hyphen_values = true)]
    spec: String,

    /// Print the validation result as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Validate(args) => validate(args),
    }
}

fn validate(args: ValidateArgs) -> ExitCode {
    let result = validate_argspec(&args.spec);
    let text = if args.json {
        json_line(&result)
    } else if let Some(slots) = &result.parsed {
        let mut text = match slots.len() {
            0 => "valid: no arguments".to_owned(),
            1 => "valid: 1 argument".to_owned(),
            count => format!("valid: {count} arguments"),
        };
        text.push_str(&describe_slots(slots));
        text
    } else {
        let error = result.error.as_deref().unwrap_or("unknown error");
        // The facade message already says `invalid argspec`.
        eprint_line(format_args!("error: {error}"));
        return ExitCode::from(FAILURE);
    };
    if let Err(error) = print_line(text) {
        return io_error(format_args!("cannot write stdout: {error}"));
    }
    if result.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(FAILURE)
    }
}

/// One indented line per slot, each preceded by a newline.
pub fn describe_slots(slots: &[ParsedArgSpecSlot]) -> String {
    slots
        .iter()
        .enumerate()
        .map(|(index, slot)| format!("\n  {}. {}", index + 1, describe_slot(slot)))
        .collect()
}

/// Summarize a slot, for example `optional, math content, delimited by [ ]`.
fn describe_slot(slot: &ParsedArgSpecSlot) -> String {
    let mut parts = vec![
        if slot.required {
            "required"
        } else {
            "optional"
        }
        .to_owned(),
        describe_kind(&slot.kind),
    ];
    match &slot.form {
        ArgSpecFormInfo::Standard | ArgSpecFormInfo::Star => {}
        ArgSpecFormInfo::Group => parts.push("braced".to_owned()),
        ArgSpecFormInfo::Delimited { open, close } => {
            parts.push(format!(
                "delimited by {} {}",
                delimiter(open),
                delimiter(close)
            ));
        }
        ArgSpecFormInfo::Paired { pairs } => {
            let pairs: Vec<String> = pairs
                .iter()
                .map(|pair| format!("{} {}", delimiter(&pair.open), delimiter(&pair.close)))
                .collect();
            parts.push(format!("delimited by {}", pairs.join(" or ")));
        }
    }
    if slot.nullable {
        parts.push("null when absent".to_owned());
    }
    if slot.no_leading_space {
        parts.push("no leading space".to_owned());
    }
    parts.join(", ")
}

fn describe_kind(kind: &ArgSpecKindInfo) -> String {
    match kind {
        ArgSpecKindInfo::Content {
            mode: RuntimeContentModeInfo::Math,
        } => "math content",
        ArgSpecKindInfo::Content {
            mode: RuntimeContentModeInfo::Text,
        } => "text content",
        ArgSpecKindInfo::OperatorName => "operator name",
        ArgSpecKindInfo::Delimiter => "delimiter",
        ArgSpecKindInfo::CsName => "control sequence name",
        ArgSpecKindInfo::Dimension => "dimension",
        ArgSpecKindInfo::Integer => "integer",
        ArgSpecKindInfo::KeyVal => "key-value list",
        ArgSpecKindInfo::Column => "column specification",
        ArgSpecKindInfo::Star => "star",
    }
    .to_owned()
}

fn delimiter(token: &DelimiterTokenInfo) -> String {
    match token {
        DelimiterTokenInfo::Char { value } => value.to_string(),
        DelimiterTokenInfo::ControlSeq { value } => format!("\\{value}"),
    }
}
