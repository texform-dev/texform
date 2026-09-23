//! `texform info`: knowledge-base entry for a command, character, or
//! environment.

use std::process::ExitCode;

use serde::Serialize;
use texform::ContentMode;
use texform::bindings::{
    CharacterInfoDto, CommandInfoDto, EnvInfoDto, character_info_to_dto, command_info_to_dto,
    env_info_to_dto,
};

use super::argspec::describe_slots;
use crate::output::{FAILURE, eprint_line, io_error, json_line, print_line, usage_error};
use crate::packages;

#[derive(clap::Args)]
pub struct Args {
    /// Control sequence such as `\frac` or `\alpha`, or an environment name with `--env`
    #[arg(value_name = "NAME", allow_hyphen_values = true)]
    name: String,

    /// Look up an environment instead of a control sequence
    #[arg(long)]
    env: bool,

    /// Content mode to look the name up in
    #[arg(long, value_enum, default_value = "math")]
    mode: Mode,

    /// Print the records as one JSON object (`null` when none is found)
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Mode {
    Math,
    Text,
}

impl From<Mode> for ContentMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Math => ContentMode::Math,
            Mode::Text => ContentMode::Text,
        }
    }
}

/// Knowledge-base records found for a name, as the bindings' info DTOs.
///
/// Character commands such as `\alpha` have a character record and also
/// parse as commands without arguments, and a few names such as `\div` are
/// an ordinary command in one package and a character in another, so a
/// control sequence reports both records.
#[derive(Serialize)]
struct Entries {
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<CommandInfoDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    character: Option<CharacterInfoDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<EnvInfoDto>,
}

impl Entries {
    fn is_empty(&self) -> bool {
        self.command.is_none() && self.character.is_none() && self.environment.is_none()
    }
}

pub fn run(args: Args, packages: &[String]) -> ExitCode {
    let parser = match packages::parser(packages) {
        Ok(parser) => parser,
        Err(error) => return usage_error(error),
    };
    let mode = ContentMode::from(args.mode);
    let entries = if args.env {
        Entries {
            command: None,
            character: None,
            environment: parser.lookup_env(&args.name, mode).map(env_info_to_dto),
        }
    } else if let Some(name) = args.name.strip_prefix('\\') {
        Entries {
            command: parser.lookup_command(name, mode).map(command_info_to_dto),
            character: parser
                .lookup_character(name, mode)
                .map(character_info_to_dto),
            environment: None,
        }
    } else {
        return usage_error(format_args!(
            "`{}` is not a control sequence; write it with a backslash, such as `\\frac`, or use `--env` for an environment",
            args.name
        ));
    };
    let found = !entries.is_empty();
    let text = if args.json {
        json_line(&found.then_some(&entries))
    } else if found {
        describe(&entries)
    } else {
        let what = if args.env {
            "environment"
        } else {
            "command or character"
        };
        let mode = match args.mode {
            Mode::Math => "math",
            Mode::Text => "text",
        };
        eprint_line(format_args!(
            "error: no {what} `{}` in {mode} mode",
            args.name
        ));
        return ExitCode::from(FAILURE);
    };
    if let Err(error) = print_line(text) {
        return io_error(format_args!("cannot write stdout: {error}"));
    }
    if found {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(FAILURE)
    }
}

/// One block of aligned fields per record, separated by blank lines.
fn describe(entries: &Entries) -> String {
    let mut blocks = Vec::new();
    if let Some(info) = &entries.command {
        let mut text = fields(&[
            ("command", format!("\\{}", info.name)),
            ("kind", info.kind.to_owned()),
            ("mode", info.allowed_mode.to_owned()),
            ("packages", info.from_packages.join(", ")),
            ("tags", info.tags.join(", ")),
            ("argspec", argspec(&info.spec_string)),
        ]);
        text.push_str(&describe_slots(&info.args));
        blocks.push(text);
    }
    if let Some(info) = &entries.character {
        blocks.push(fields(&[
            ("character", format!("\\{}", info.name)),
            ("unicode", info.unicode_value.clone()),
            ("mode", info.allowed_mode.to_owned()),
            (
                "variant",
                info.attributes.mathvariant.clone().unwrap_or_default(),
            ),
            ("package", info.package.clone()),
        ]));
    }
    if let Some(info) = &entries.environment {
        let mut text = fields(&[
            ("environment", info.name.clone()),
            ("mode", info.allowed_mode.to_owned()),
            ("body", info.body_mode.to_owned()),
            ("packages", info.from_packages.join(", ")),
            ("tags", info.tags.join(", ")),
            ("argspec", argspec(&info.spec_string)),
        ]);
        text.push_str(&describe_slots(&info.args));
        blocks.push(text);
    }
    blocks.join("\n\n")
}

fn argspec(spec: &str) -> String {
    if spec.is_empty() {
        "(no arguments)".to_owned()
    } else {
        spec.to_owned()
    }
}

/// Aligned `key: value` lines, skipping empty values.
fn fields(rows: &[(&str, String)]) -> String {
    let width = rows.iter().map(|(key, _)| key.len()).max().unwrap_or(0) + 1;
    rows.iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| format!("{:width$} {value}", format!("{key}:")))
        .collect::<Vec<_>>()
        .join("\n")
}
